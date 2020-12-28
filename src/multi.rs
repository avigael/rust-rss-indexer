use rss::Channel;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use std::result::Result;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use url::Url;

use crate::common::*;

/// Thread limits.
const MAX_THREADS_FEEDS: u32 = 5;
const MAX_THREADS_SITES: u32 = 10;
const MAX_THREADS_TOTAL: u32 = 18;

/// A lock around some T, with a condition variable for notifying/waiting.
struct CvarLock<T> {
    mutex: Mutex<T>,
    condvar: Condvar,
}

impl<T> CvarLock<T> {
    fn new(data: T) -> Self {
        let mutex = Mutex::new(data);
        let condvar = Condvar::new();
        CvarLock { mutex, condvar }
    }
}

/// Locks/Condvars around counters, tracking the number of feed threads, the number of article
/// threads per hostname, and the total number of threads.
pub struct ThreadCount {
    feeds_count: CvarLock<u32>,
    sites_count: CvarLock<HashMap<String, u32>>,
    total_count: CvarLock<u32>,
}

/// Same as for the single-threaded version, but now spawn a new thread for each call to
/// `process_feed`. Make sure to respect the thread limits!
pub fn process_feed_file(file_name: &str, index: Arc<Mutex<ArticleIndex>>) -> RssIndexResult<()> {
    let file = File::open(file_name)?;
    println!("Processing feed file: {}", file_name);

    let channel = Channel::read_from(BufReader::new(file))?;

    let urls = Arc::new(Mutex::new(HashSet::new()));

    let counters = Arc::new(ThreadCount {
        feeds_count: CvarLock::new(0),
        sites_count: CvarLock::new(HashMap::new()),
        total_count: CvarLock::new(0),
    });

    let mut threads = Vec::new();

    for feed in channel.into_items() {
        let url = feed.link().ok_or(RssIndexError::UrlError)?;
        let title = feed.title().ok_or(RssIndexError::UrlError)?;
        let url_string: String = url.to_string();
        let my_index = Arc::clone(&index);
        let my_urls = urls.clone();
        let my_counter = counters.clone();

        {
            let mut urll = my_urls.lock().unwrap();
            if urll.contains(url) {
                println!("Skipping already seen feed: {} [{}]", title, url);
                continue;
            }
            urll.insert(url.to_string());
        }

        {
            let mut feeds = counters.feeds_count.mutex.lock().unwrap();
            let mut totals = counters.total_count.mutex.lock().unwrap();

            while *feeds >= MAX_THREADS_FEEDS {
                feeds = counters.feeds_count.condvar.wait(feeds).unwrap();
            }
            *feeds += 1;

            while *totals >= MAX_THREADS_TOTAL {
                totals = counters.total_count.condvar.wait(totals).unwrap();
            }
            *totals += 1;
        }

        println!("Processing feed: {} [{}]", title, url);
        let thread = thread::spawn(move || {
            let my_counter_clone = my_counter.clone();
            process_feed(&url_string, my_index, my_urls, my_counter_clone)
                .expect("process_feed error");
            let mut feeds = my_counter.feeds_count.mutex.lock().unwrap();
            let mut totals = my_counter.total_count.mutex.lock().unwrap();
            *feeds -= 1;
            *totals -= 1;
            my_counter.feeds_count.condvar.notify_one();
            my_counter.total_count.condvar.notify_one();
        });
        threads.push(thread);
    }
    for thread in threads {
        thread.join().expect("Failed to join thread");
    }
    Result::Ok(())
}

/// Same as for the single-threaded version, but now spawn a new thread for each call to
/// `process_article`. Make sure to respect the thread limits!
fn process_feed(
    url: &str,
    index: Arc<Mutex<ArticleIndex>>,
    urls: Arc<Mutex<HashSet<String>>>,
    counters: Arc<ThreadCount>,
) -> RssIndexResult<()> {
    let contents = reqwest::blocking::get(url)?.bytes()?;
    let channel = Channel::read_from(&contents[..])?;
    let items = channel.into_items();
    let mut threads = Vec::new();

    for item in items {
        let (url, site, title) = match (item.link(), Url::parse(&url)?.host_str(), item.title()) {
            (Some(u), Some(s), Some(t)) => (u, s.to_string(), t),
            _ => continue,
        };
        let my_urls = urls.clone();
        let url_string: String = url.to_string();
        let title_string: String = title.to_string();
        let my_site = site.clone();
        let my_counter = counters.clone();
        let my_index = index.clone();
        {
            let mut urll = my_urls.lock().unwrap();
            if urll.contains(url) {
                println!("Skipping already seen article: {} [{}]", title, url);
                continue;
            }
            urll.insert(url.to_string());
        }
        {
            let mut sites = counters.sites_count.mutex.lock().unwrap();

            if !sites.contains_key(&site) {
                sites.insert(site, 0);
            }

            while sites.get(&my_site).unwrap() >= &MAX_THREADS_SITES {
                sites = counters.sites_count.condvar.wait(sites).unwrap();
            }
            *sites.get_mut(&my_site).unwrap() += 1;
        }
        {
            let mut totals = counters.total_count.mutex.lock().unwrap();

            while *totals >= MAX_THREADS_TOTAL {
                totals = counters.total_count.condvar.wait(totals).unwrap();
            }
            *totals += 1;
        }
        println!("Processing article: {} [{}]", title, url);
        let thread = thread::spawn(move || {
            let article = Article::new(url_string.to_string(), title_string.to_string());
            let article_words = process_article(&article);

            my_index.lock().unwrap().add(
                my_site.to_string(),
                title_string.to_string(),
                url_string.to_string(),
                article_words.unwrap(),
            );

            let mut sites = my_counter.sites_count.mutex.lock().unwrap();
            let mut totals = my_counter.total_count.mutex.lock().unwrap();
            *sites.get_mut(&my_site).unwrap() -= 1;
            *totals -= 1;
            my_counter.sites_count.condvar.notify_one();
            my_counter.total_count.condvar.notify_one();
        });
        threads.push(thread);
    }
    for thread in threads {
        thread.join().expect("Failed to join thread");
    }
    Result::Ok(())
}
