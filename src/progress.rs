use indicatif;
use std::env;

pub type Progress = indicatif::ProgressBar;
pub type MultiProgress = indicatif::MultiProgress;
pub type ProgressStyle = indicatif::ProgressStyle;

pub fn new_multi_progress() -> MultiProgress {
    let silent: bool = env::var("SILENT").unwrap() == "true";
    let m = MultiProgress::new();
    if silent {
        m.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        return m;
    }
    m
}

pub fn new(total: u64, style: &str) -> Progress {
    let mut pb = Progress::new(total);
    let silent: bool = env::var("SILENT").unwrap() == "true";
    if silent {
        pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
        return pb;
    }
    match style {
        "file_transfer" => pb = set_style_file_transfer_units(pb),
        "migration_progress" => pb = set_style_migration_progress_units(pb),
        unknown => eprintln!("Unknown style: {}", unknown),
    }
    pb
}

pub fn set_style_file_transfer_units(pb: Progress) -> Progress {
    pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .unwrap()
            .progress_chars("█  "));
    pb
}

pub fn set_style_migration_progress_units(pb: Progress) -> Progress {
    pb.set_style(ProgressStyle::default_bar()
            .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {pos}/{len} ({percent}%)")
            .unwrap()
            .progress_chars("█  "));
    pb
}
