// use indicatif::ProgressBar;
// use indicatif::{ProgressBar, ProgressStyle};
// use std::env;

// pub type Progress = ProgressBar;

// pub fn new(total: u64) -> Progress {
//     let pb = Progress::new(total);
//     if env::var("SILENT").unwrap() == "true" {
//         pb.set_draw_target(indicatif::ProgressDrawTarget::hidden());
//     } else {
//         pb.set_draw_target(indicatif::ProgressDrawTarget::stdout());
//         pb.set_position(0);
//     }
//     pb
// }

// pub fn set_style_file_transfer_units(pb: Progress) -> Progress {
//     pb.set_style(ProgressStyle::default_bar()
//             .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
//             .unwrap()
//             .progress_chars("█  "));
//     pb
// }

// pub fn set_style_migration_progress_units(pb: Progress) -> Progress {
//     pb.set_style(ProgressStyle::default_bar()
//             .template("{msg}\n{spinner:.green}  [{elapsed_precise}] [{wide_bar:.white/blue}] {pos}/{len} ({percent}%)")
//             .unwrap()
//             .progress_chars("█  "));
//     pb
// }
