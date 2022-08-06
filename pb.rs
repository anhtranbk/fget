pub fn test_show_pb() {
    let mut downloaded: u32 = 0;
    let total_size: u32 = 243 * 1024 * 1024;

    let pb = ProgressBar::new(total_size as u64);
    pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap());
    // .progress_chars("#>-"));

    while downloaded < total_size {
        let new = 750 * 1024;
        downloaded = min(downloaded + new, total_size);

        pb.set_position(downloaded as u64);
        // pb.inc(new as u64);
        thread::sleep(Duration::from_millis(100));
    }

    pb.finish_with_message("downloaded")
}
