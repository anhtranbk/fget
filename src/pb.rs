use std::{cmp::min, thread, time::Duration};

use crate::downloader::DownloadObserver;
use indicatif::{ProgressBar, ProgressStyle};

pub struct ProgressManager {
    pbs: Vec<ProgressBar>,
}

impl DownloadObserver for ProgressManager {
    fn on_download_start(&mut self, _: u8, len: u64) {
        self.pbs.push(new_progress_bar(len));
    }

    fn on_progress(&mut self, idx: u8, pos: u64) {
        // println!("on progress for {}: {:?}", idx, pos);
        if let Some(pb) = self.pbs.get_mut(idx as usize) {
            pb.set_position(pos);
        }
    }

    fn on_download_end(&mut self, idx: u8) {
        if let Some(pb) = self.pbs.get_mut(idx as usize) {
            pb.finish_with_message(format!("part {} downloaded", idx));
        }
    }

    fn on_message(&mut self, msg: &str) {
        println!("on message {}", msg);
    }
}

impl ProgressManager {
    pub fn new(size: usize) -> Self {
        Self {
            pbs: Vec::with_capacity(size),
        }
    }
}

fn new_progress_bar(len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len as u64);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({binary_bytes_per_sec}, {eta})")
        .unwrap());
    // .progress_chars("#>-"));

    pb
}

#[allow(dead_code)]
pub fn test_show_pb() {
    let mut downloaded = 0u64;
    let len: u64 = 243 * 1024 * 1024;
    let pb = new_progress_bar(len);

    while downloaded < len {
        let new = 750 * 1024;
        downloaded = min(downloaded + new, len);

        // pb.inc(new as u64);
        pb.set_position(downloaded as u64);
        thread::sleep(Duration::from_millis(100));
    }

    pb.finish_with_message("downloaded")
}
