use rss::Channel;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::result::Result;
use std::sync::{Arc, Mutex};
use url::Url;

use crate::common::*;
use crate::threadpool::*;

/// Thread pool sizes.
const SIZE_FEEDS_POOL: usize = 3;
const SIZE_SITES_POOL: usize = 20;

/// Same as the single/multi threaded version, but using a thread pool. Set up two thread pools:
/// one for handling feeds, and one for handling articles. Use the sizes above. Push closures
/// executing `process_feed` into the thread pool.
pub fn process_feed_file(file_name: &str, index: Arc<Mutex<ArticleIndex>>) -> RssIndexResult<()> {
    let file = File::open(file_name)?;
    println!("Processing feed file: {}", file_name);

    let channel = Channel::read_from(BufReader::new(file))?;
    let urls = Arc::new(Mutex::new(HashSet::new()));

    for feed in channel.into_items() {
        let url = feed.link().ok_or(RssIndexError::UrlError)?;
        let title = feed.title().ok_or(RssIndexError::UrlError)?;
        let url_string: String = url.to_string();
        let my_urls = urls.clone();
        let my_index = Arc::clone(&index);

        {
            let mut urll = my_urls.lock().unwrap();
            if urll.contains(url) {
                println!("Skipping already seen feed: {} [{}]", title, url);
                continue;
            }
            urll.insert(url.to_string());
        }

        println!("Processing feed: {} [{}]", title, url);
        let feeds_pool = Arc::new(Mutex::new(ThreadPool::new(SIZE_FEEDS_POOL)));
        let sites_pool = Arc::new(Mutex::new(ThreadPool::new(SIZE_SITES_POOL)));

        feeds_pool.lock().unwrap().execute(move || {
            process_feed(&url_string, my_index, my_urls, sites_pool).expect("process_feed error");
        });
    }
    Result::Ok(())
}

/// Same as the single/multi threaded version, but using a thread pool. Push closures executing
/// `process_article` into the thread pool that is passed in.
fn process_feed(
    url: &str,
    index: Arc<Mutex<ArticleIndex>>,
    urls: Arc<Mutex<HashSet<String>>>,
    sites_pool: Arc<Mutex<ThreadPool>>,
) -> RssIndexResult<()> {
    let contents = reqwest::blocking::get(url)?.bytes()?;
    let channel = Channel::read_from(&contents[..])?;
    let items = channel.into_items();

    for item in items {
        let (url, site, title) = match (item.link(), Url::parse(&url)?.host_str(), item.title()) {
            (Some(u), Some(s), Some(t)) => (u, s.to_string(), t),
            _ => continue,
        };

        let my_urls = urls.clone();
        let url_string: String = url.to_string();
        let title_string: String = title.to_string();
        let my_site = site.clone();
        let my_index = index.clone();

        {
            let mut urll = my_urls.lock().unwrap();
            if urll.contains(url) {
                println!("Skipping already seen article: {} [{}]", title, url);
                continue;
            }
            urll.insert(url.to_string());
        }

        println!("Processing article: {} [{}]", title, url);
        sites_pool.lock().unwrap().execute(move || {
            let article = Article::new(url_string.to_string(), title_string.to_string());
            let article_words = process_article(&article);

            my_index.lock().unwrap().add(
                my_site.to_string(),
                title_string.to_string(),
                url_string.to_string(),
                article_words.unwrap(),
            );
        });
    }
    Result::Ok(())
}
