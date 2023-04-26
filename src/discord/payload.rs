pub struct Payload {
    pub details: String,
    pub state: String,
    pub media: String,
    pub link_imdb: String,
    pub link_trakt: String,
    pub img_url: String,
    pub watch_percentage: String,
}

impl Default for Payload {
    fn default() -> Self {
        Payload {
            details: String::from(""),
            state: String::from(""),
            media: String::from(""),
            link_imdb: String::from(""),
            link_trakt: String::from(""),
            img_url: String::from(""),
            watch_percentage: String::from(""),
        }
    }
}
