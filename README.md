# Concurrent RSS Indexer

This project is written in Rust. To run this project please [install](https://www.rust-lang.org/tools/install "install") Rust on your machine.

This is a **multithreaded**, **thread-pool**, and **singlethreaded** indexer for RSS feeds. An RSS file is an XML file (see feeds/ for two examples), in which each item points to an RSS feed somewhere on the internet. Each RSS feed is then another XML file containing a list of items (say, news articles), each with a title and a URL. This indexer will download each of these articles and build a big map of word counts, letting you query the articles for where a given word appears the most frequently. Note: A thread pool allows a client program to farm out jobs to a pool of *worker* threads. See [here](https://docs.rs/threadpool/1.8.1/threadpool/ "here") for more about threadpools in Rust.

**Instructions**: To run this file with cargo, execute the following command:
```
$ cargo run feeds/<small or medium>-feed.xml <single or multi or pooled>
```
**Example**: runs the program multithreaded for small-feed.xml file.
```
$ cargo run feeds/small-feed.xml multi
```

## External crates

* [rss](https://crates.rs/crates/rss). Crate for working with RSS feeds
* [reqwest](https://crates.rs/crates/reqwest). A high-level HTTP crate. Used this for grabbing data from web pages.
* [scraper](https://crates.rs/crates/scraper). A crate for parsing and querying HTML, built on the lower-level crate `html5ever` used by Mozilla's Servo project. Used this for parsing HTML and querying tags (e.g., grabbing the contents of the "<body>" tag)
* [url](https://crates.rs/crates/url). A crate for working with URLs: parsing, getting hostname, etc.
