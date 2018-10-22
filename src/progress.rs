use indicatif;


pub enum ProgressBar {
    Visual(indicatif::ProgressBar),
    Nop,
}

impl ProgressBar {
    pub fn set_size(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_length(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn set_position(&self, size: u64) {
        match self {
            ProgressBar::Visual(pb) => pb.set_position(size),
            ProgressBar::Nop => {}
        }
    }

    pub fn end(&self) {
        match self {
            ProgressBar::Visual(pb) => pb.finish(),
            ProgressBar::Nop => {}
        }
    }
}


pub fn iprogress_bar(size: u64) -> ProgressBar {
    let ipb = indicatif::ProgressBar::new(size);
    ipb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:80.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"),
    );
    ProgressBar::Visual(ipb)
}
