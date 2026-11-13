use eframe::egui::Color32;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Sending,
    Sent,
    Delivered,
    Read,
}

#[derive(Clone)]
pub enum Body {
    Text(String),
    Photo {
        caption: String,
        tint: Color32,
    },
    Document {
        name: String,
        size: String,
    },
    Voice {
        secs: u32,
    },
    Location {
        place: String,
    },
}

#[derive(Clone)]
pub struct Message {
    pub body: Body,
    pub out: bool,
    pub time: String,
    pub day: String,
    pub status: Status,
    pub sender: Option<String>,
}

impl Message {
    pub fn text(&self) -> String {
        match &self.body {
            Body::Text(t) => t.clone(),
            Body::Photo { caption, .. } => {
                if caption.is_empty() {
                    "Photo".to_owned()
                } else {
                    format!("Photo · {caption}")
                }
            }
            Body::Document { name, .. } => name.clone(),
            Body::Voice { secs } => format!("Voice message · {}:{:02}", secs / 60, secs % 60),
            Body::Location { place } => place.clone(),
        }
    }

    pub fn preview(&self) -> String {
        match &self.body {
            Body::Text(t) => t.clone(),
            Body::Photo { .. } => "Photo".to_owned(),
            Body::Document { name, .. } => format!("Document · {name}"),
            Body::Voice { secs } => format!("Voice message ({}:{:02})", secs / 60, secs % 60),
            Body::Location { place } => format!("Location · {place}"),
        }
    }
}

pub struct Chat {
    pub id: u64,
    pub name: String,
    pub initials: String,
    pub color: Color32,
    pub messages: Vec<Message>,
    pub pinned: bool,
    pub muted: bool,
    pub unread: usize,
    pub online: bool,
    pub last_seen: String,
    pub about: String,
    pub phone: String,
    pub is_group: bool,
    pub members: usize,
    pub typing: bool,
    pub draft: String,
    pub updated: u64,
}

impl Chat {
    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    pub fn preview(&self) -> String {
        match self.messages.last() {
            Some(m) => {
                if m.out {
                    format!("You: {}", m.preview())
                } else if self.is_group {
                    if let Some(sender) = &m.sender {
                        format!("{}: {}", sender, m.preview())
                    } else {
                        m.preview()
                    }
                } else {
                    m.preview()
                }
            }
            None => "Say hello".to_owned(),
        }
    }

    pub fn time_label(&self) -> String {
        match self.messages.last() {
            Some(m) => {
                if m.day == "Today" {
                    m.time.clone()
                } else if m.day == "Yesterday" {
                    "Yesterday".to_owned()
                } else {
                    m.day.clone()
                }
            }
            None => String::new(),
        }
    }

    pub fn status_line(&self) -> String {
        if self.typing {
            "typing…".to_owned()
        } else if self.is_group {
            let names = ["Maya", "Sam", "Priya", "Alex", "Noah"];
            format!(
                "{} · {} members",
                names[..3.min(names.len())].join(", "),
                self.members
            )
        } else if self.online {
            "online".to_owned()
        } else {
            self.last_seen.clone()
        }
    }
}

fn m(body: Body, out: bool, time: &str, day: &str, status: Status) -> Message {
    Message {
        body,
        out,
        time: time.to_owned(),
        day: day.to_owned(),
        status,
        sender: None,
    }
}

fn text(t: &str, out: bool, time: &str, day: &str, status: Status) -> Message {
    m(Body::Text(t.to_owned()), out, time, day, status)
}

pub fn sample_chats() -> Vec<Chat> {
    let mut chats = vec![
        Chat {
            id: 1,
            name: "Maya Chen".into(),
            initials: "MC".into(),
            color: Color32::from_rgb(226, 148, 118),
            pinned: true,
            muted: false,
            unread: 2,
            online: true,
            last_seen: "online".into(),
            about: "Product designer. Probably sketching something.".into(),
            phone: "+1 555 0142".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 100,
            messages: vec![
                text(
                    "Did you see the new mockups? I finally fixed the spacing around the composer.",
                    false,
                    "16:48",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Just opened them — so much calmer now. The thread feels roomy.",
                    true,
                    "16:52",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "That was the goal. I also softened the dividers a little.",
                    false,
                    "16:54",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Morning! Ready for the 10:30 review?",
                    false,
                    "08:56",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Yep. Coffee first ☕",
                    true,
                    "09:03",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Ha. Same. I'll send the link in a minute.",
                    false,
                    "09:04",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Also — have a look at the dark theme when you get a second. It's finally coming together.",
                    false,
                    "09:12",
                    "Today",
                    Status::Read,
                ),
                text(
                    "And thank you for keeping this build so tiny. It opens instantly on my old laptop.",
                    false,
                    "09:13",
                    "Today",
                    Status::Read,
                ),
            ],
        },
        Chat {
            id: 2,
            name: "Weekend Hikers".into(),
            initials: "WH".into(),
            color: Color32::from_rgb(112, 168, 108),
            pinned: false,
            muted: true,
            unread: 5,
            online: false,
            last_seen: "last seen yesterday at 21:18".into(),
            about: "Saturday trails, good snacks, no rush.".into(),
            phone: "6 members".into(),
            is_group: true,
            members: 6,
            typing: false,
            draft: String::new(),
            updated: 98,
            messages: vec![
                text(
                    "Trail is open again after the storm.",
                    false,
                    "18:02",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Saturday 8am at the north gate?",
                    true,
                    "18:10",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Works for me. I'll bring the good thermos.",
                    false,
                    "18:12",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Forecast looks perfect — cool and clear.",
                    false,
                    "08:21",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Count me in. Bringing oat cookies.",
                    false,
                    "08:24",
                    "Today",
                    Status::Read,
                ),
                text(
                    "If anyone needs a lift, I have two seats.",
                    false,
                    "08:31",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Perfect. See you all at eight.",
                    false,
                    "08:33",
                    "Today",
                    Status::Read,
                ),
            ],
        },
        Chat {
            id: 3,
            name: "Daniel Okafor".into(),
            initials: "DO".into(),
            color: Color32::from_rgb(92, 141, 246),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen today at 09:12".into(),
            about: "Illustrator. Usually covered in ink.".into(),
            phone: "+1 555 0187".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 96,
            messages: vec![
                text(
                    "Sent you the invoice for the cover illustration.",
                    true,
                    "09:02",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Got it, thanks! Paying today.",
                    false,
                    "09:10",
                    "Today",
                    Status::Read,
                ),
                text("No rush 👍", true, "09:12", "Today", Status::Read),
            ],
        },
        Chat {
            id: 4,
            name: "Mum".into(),
            initials: "M".into(),
            color: Color32::from_rgb(224, 118, 118),
            pinned: false,
            muted: false,
            unread: 1,
            online: false,
            last_seen: "last seen today at 08:40".into(),
            about: "Retired teacher. Chief supplier of soup.".into(),
            phone: "+1 555 0110".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 94,
            messages: vec![
                text("Landed safely!", true, "21:14", "Yesterday", Status::Read),
                text(
                    "Good. Get some rest.",
                    false,
                    "21:16",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Call me when you're free ❤",
                    false,
                    "08:40",
                    "Today",
                    Status::Read,
                ),
            ],
        },
        Chat {
            id: 5,
            name: "Design Crew".into(),
            initials: "DC".into(),
            color: Color32::from_rgb(238, 166, 92),
            pinned: false,
            muted: true,
            unread: 0,
            online: false,
            last_seen: "9 members".into(),
            about: "Works in progress, kind feedback, good links.".into(),
            phone: "9 members".into(),
            is_group: true,
            members: 9,
            typing: false,
            draft: String::new(),
            updated: 92,
            messages: vec![
                text(
                    "New icon set is in Figma.",
                    false,
                    "11:04",
                    "Today",
                    Status::Read,
                ),
                text(
                    "Sharing notes after lunch.",
                    false,
                    "11:06",
                    "Today",
                    Status::Read,
                ),
                text(
                    "The lighter strokes read much better at small sizes.",
                    false,
                    "11:08",
                    "Today",
                    Status::Read,
                ),
            ],
        },
        Chat {
            id: 6,
            name: "Priya Shah".into(),
            initials: "PS".into(),
            color: Color32::from_rgb(176, 124, 232),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen yesterday at 19:52".into(),
            about: "Writes about cities and the people in them.".into(),
            phone: "+1 555 0164".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 90,
            messages: vec![
                text(
                    "That article you sent was great.",
                    false,
                    "19:48",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Knew you'd like it. The bit about benches stayed with me.",
                    true,
                    "19:50",
                    "Yesterday",
                    Status::Read,
                ),
                text("Exactly that part.", false, "19:52", "Yesterday", Status::Read),
            ],
        },
        Chat {
            id: 7,
            name: "Alex Rivera".into(),
            initials: "AR".into(),
            color: Color32::from_rgb(77, 182, 172),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen yesterday at 17:05".into(),
            about: "Photographer. Chases the early light.".into(),
            phone: "+1 555 0129".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 88,
            messages: vec![
                text("How was the coast?", true, "16:40", "Yesterday", Status::Read),
                m(
                    Body::Photo {
                        caption: "Unreal light this morning.".into(),
                        tint: Color32::from_rgb(106, 164, 186),
                    },
                    false,
                    "17:02",
                    "Yesterday",
                    Status::Read,
                ),
                text("Unreal light this morning.", false, "17:03", "Yesterday", Status::Read),
                text("Worth the 5am alarm.", false, "17:05", "Yesterday", Status::Read),
            ],
        },
        Chat {
            id: 8,
            name: "Sam Whitfield".into(),
            initials: "SW".into(),
            color: Color32::from_rgb(141, 154, 168),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen yesterday at 22:30".into(),
            about: "Building small tools that stay out of the way.".into(),
            phone: "+1 555 0193".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 86,
            messages: vec![
                text(
                    "Congrats on the launch!",
                    true,
                    "22:20",
                    "Yesterday",
                    Status::Read,
                ),
                text(
                    "Thanks! Long week but worth it.",
                    false,
                    "22:28",
                    "Yesterday",
                    Status::Read,
                ),
                text("Celebrate properly this weekend.", true, "22:30", "Yesterday", Status::Read),
            ],
        },
        Chat {
            id: 9,
            name: "Noah Park".into(),
            initials: "NP".into(),
            color: Color32::from_rgb(108, 155, 210),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen today at 08:02".into(),
            about: "Makes music, forgets to answer texts.".into(),
            phone: "+1 555 0175".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 84,
            messages: vec![
                text(
                    "Free for a quick sync tomorrow?",
                    false,
                    "08:00",
                    "Today",
                    Status::Read,
                ),
                text("Morning works.", true, "08:02", "Today", Status::Read),
            ],
        },
        Chat {
            id: 10,
            name: "Lena Fischer".into(),
            initials: "LF".into(),
            color: Color32::from_rgb(217, 139, 168),
            pinned: false,
            muted: false,
            unread: 0,
            online: false,
            last_seen: "last seen Monday at 15:40".into(),
            about: "Bookbinder. Likes long walks and short emails.".into(),
            phone: "+1 555 0158".into(),
            is_group: false,
            members: 0,
            typing: false,
            draft: String::new(),
            updated: 82,
            messages: vec![
                text(
                    "See you at the thing on Friday.",
                    false,
                    "15:38",
                    "Monday",
                    Status::Read,
                ),
                text("Wouldn't miss it.", true, "15:40", "Monday", Status::Read),
            ],
        },
    ];

    for chat in chats.iter_mut().filter(|c| c.is_group) {
        let names: &[&str] = if chat.id == 2 {
            &["Sam", "Maya", "Priya", "Alex", "Noah"]
        } else {
            &["Nora", "Theo", "Maya", "Iris"]
        };
        let mut i = 0;
        for msg in chat.messages.iter_mut() {
            if !msg.out {
                msg.sender = Some(names[i % names.len()].to_string());
                i += 1;
            }
        }
    }

    chats
}

pub struct ActiveCall {
    pub name: String,
    pub initials: String,
    pub color: Color32,
    pub video: bool,
    pub started: f64,
    pub muted: bool,
    pub answered: Option<f64>,
}

pub struct CallEntry {
    pub name: String,
    pub initials: String,
    pub color: Color32,
    pub when: String,
    pub incoming: bool,
    pub video: bool,
    pub missed: bool,
}

pub fn sample_calls() -> Vec<CallEntry> {
    vec![
        CallEntry {
            name: "Maya Chen".into(),
            initials: "MC".into(),
            color: Color32::from_rgb(226, 148, 118),
            when: "Today, 08:12".into(),
            incoming: true,
            video: true,
            missed: false,
        },
        CallEntry {
            name: "Mum".into(),
            initials: "M".into(),
            color: Color32::from_rgb(224, 118, 118),
            when: "Yesterday, 21:20".into(),
            incoming: true,
            video: false,
            missed: true,
        },
        CallEntry {
            name: "Daniel Okafor".into(),
            initials: "DO".into(),
            color: Color32::from_rgb(92, 141, 246),
            when: "Yesterday, 14:02".into(),
            incoming: false,
            video: false,
            missed: false,
        },
        CallEntry {
            name: "Design Crew".into(),
            initials: "DC".into(),
            color: Color32::from_rgb(238, 166, 92),
            when: "Monday, 11:30".into(),
            incoming: false,
            video: true,
            missed: false,
        },
    ]
}
