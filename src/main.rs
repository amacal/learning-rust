use std::borrow::Cow;
use std::collections::HashSet;
use std::error::Error;

use std::fs;
use std::fs::File;

use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::sync::Arc;

use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;

use quick_xml::name::LocalName;
use quick_xml::reader::Reader;
use sevenz_rust::Password;
use sevenz_rust::SevenZReader;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

macro_rules! with_functions {
    ($($name:ident,)*) => {
        paste::paste! {
            $(
                fn [<with_ $name>](mut self, value: String) -> Self {
                    self.$name = Some(value);
                    self
                }
            )*
        }
    };
}

#[derive(serde::Serialize)]
struct RevisionInfo {
    timestamp: String,
    page_id: String,
    page_title: String,
    namespace_id: String,
    revision_id: String,
    revision_id_prev: Option<String>,
    revision_sha1: Option<String>,
    contributor_id: Option<String>,
    contributor_ip: Option<String>,
    contributor_name: Option<String>,
}

impl RevisionInfo {
    fn from(page: ParserPage, revision: ParserRevision) -> Self {
        Self {
            timestamp: revision.timestamp.unwrap(),
            page_id: page.page_id.unwrap(),
            page_title: page.title.unwrap(),
            namespace_id: page.namespace_id.unwrap(),
            revision_id: revision.revision_id.unwrap(),
            revision_id_prev: revision.revision_id_parent,
            revision_sha1: revision.sha1,
            contributor_id: revision.contributor_id,
            contributor_ip: revision.contributor_ip,
            contributor_name: revision.contributor_name,
        }
    }

    fn as_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[derive(Debug, Clone)]
struct ParserPage {
    page_id: Option<String>,
    namespace_id: Option<String>,
    title: Option<String>,
}

impl Default for ParserPage {
    fn default() -> Self {
        Self {
            page_id: None,
            namespace_id: None,
            title: None,
        }
    }
}

impl ParserPage {
    with_functions! {
        page_id,
        namespace_id,
        title,
    }
}

#[derive(Debug, Clone)]
struct ParserRevision {
    revision_id: Option<String>,
    revision_id_parent: Option<String>,
    sha1: Option<String>,
    timestamp: Option<String>,
    contributor_id: Option<String>,
    contributor_ip: Option<String>,
    contributor_name: Option<String>,
}

impl ParserRevision {
    with_functions! {
        revision_id,
        revision_id_parent,
        sha1,
        timestamp,
        contributor_id,
        contributor_ip,
        contributor_name,
    }
}

impl Default for ParserRevision {
    fn default() -> Self {
        Self {
            revision_id: None,
            revision_id_parent: None,
            sha1: None,
            timestamp: None,
            contributor_id: None,
            contributor_ip: None,
            contributor_name: None,
        }
    }
}

#[derive(Debug)]
struct ParserStateOutsideVariant {
    depth: usize,
}

impl ParserStateOutsideVariant {
    fn new(depth: usize) -> Self {
        ParserStateOutsideVariant { depth: depth }
    }

    fn increment(mut self) -> Self {
        self.depth += 1;
        self
    }

    fn decrement(mut self) -> Self {
        self.depth -= 1;
        self
    }

    fn forward(self, node: BytesStart) -> ParserState {
        match (self.depth, node.local_name()) {
            (1, node) if node.into_inner() == b"page" => {
                ParserState::InsidePage(ParserStateInsidePageVariant::new(2, None))
            }
            _ => ParserState::Outside(self.increment()),
        }
    }

    fn backward(self) -> ParserState {
        ParserState::Outside(self.decrement())
    }
}

#[derive(Debug)]
struct ParserStateInsidePageVariant {
    depth: usize,
    depth_in: usize,
    tag: Option<Vec<u8>>,
    page: ParserPage,
}

impl ParserStateInsidePageVariant {
    fn new(depth: usize, page: Option<ParserPage>) -> Self {
        ParserStateInsidePageVariant {
            depth: depth,
            depth_in: 0,
            tag: None,
            page: if let Some(page) = page {
                page
            } else {
                ParserPage::default()
            },
        }
    }

    fn increment(mut self, node: LocalName) -> Self {
        self.depth += 1;
        self.depth_in += 1;
        self.tag = Some(node.into_inner().to_vec());
        self
    }

    fn decrement(mut self) -> Self {
        self.depth -= 1;
        self.depth_in -= 1;
        self.tag = None;
        self
    }

    fn forward(self, node: BytesStart) -> ParserState {
        match (self.depth_in, node.local_name()) {
            (0, node) if node.into_inner() == b"revision" => {
                ParserState::InsideRevision(ParserStateInsideRevisionVariant::new(self.depth + 1, self.page))
            }
            (_, node) => ParserState::InsidePage(self.increment(node)),
        }
    }

    fn backward(self) -> ParserState {
        match self.depth_in {
            0 => ParserState::Outside(ParserStateOutsideVariant::new(self.depth - 1)),
            _ => ParserState::InsidePage(self.decrement()),
        }
    }

    fn process(mut self, node: BytesText) -> ParserState {
        self.page = match (self.depth_in, &self.tag) {
            (1, Some(tag)) => match tag.as_slice() {
                b"id" => self.page.with_page_id(parse_text(node)),
                b"ns" => self.page.with_namespace_id(parse_text(node)),
                b"title" => self.page.with_title(parse_text(node)),
                _ => self.page,
            },
            _ => self.page,
        };

        ParserState::InsidePage(self)
    }
}

#[derive(Debug)]
struct ParserStateInsideRevisionVariant {
    depth: usize,
    depth_in: usize,
    tag: Option<Vec<u8>>,
    in_contributor: bool,
    page: ParserPage,
    revision: ParserRevision,
}

impl ParserStateInsideRevisionVariant {
    fn new(depth: usize, page: ParserPage) -> Self {
        Self {
            depth: depth,
            depth_in: 0,
            page: page,
            tag: None,
            in_contributor: false,
            revision: ParserRevision::default(),
        }
    }

    fn increment(mut self, node: LocalName) -> Self {
        self.depth += 1;
        self.depth_in += 1;
        self.tag = Some(node.into_inner().to_vec());
        self
    }

    fn decrement(mut self) -> Self {
        self.depth -= 1;
        self.depth_in -= 1;
        self.tag = None;
        self
    }

    fn forward(mut self, node: BytesStart) -> ParserState {
        match (self.depth_in, node.local_name()) {
            (0, node) if node.into_inner() == b"contributor" => {
                self.in_contributor = true;
                ParserState::InsideRevision(self.increment(node))
            }
            (_, node) => ParserState::InsideRevision(self.increment(node)),
        }
    }

    fn backward(mut self, node: BytesEnd) -> ParserState {
        match (self.depth_in, node.local_name()) {
            (0, _) => ParserState::InsidePage(ParserStateInsidePageVariant::new(self.depth - 1, Some(self.page))),
            (1, node) if node.into_inner() == b"contributor" => {
                self.in_contributor = false;
                ParserState::InsideRevision(self.decrement())
            }
            _ => ParserState::InsideRevision(self.decrement()),
        }
    }

    fn process(mut self, node: BytesText) -> ParserState {
        self.revision = match (self.in_contributor, self.depth_in, &self.tag) {
            (false, 1, Some(tag)) => match tag.as_slice() {
                b"id" => self.revision.with_revision_id(parse_text(node)),
                b"parentid" => self.revision.with_revision_id_parent(parse_text(node)),
                b"sha1" => self.revision.with_sha1(parse_text(node)),
                b"timestamp" => self.revision.with_timestamp(parse_text(node)),
                _ => self.revision,
            },
            (true, 2, Some(tag)) => match tag.as_slice() {
                b"id" => self.revision.with_contributor_id(parse_text(node)),
                b"ip" => self.revision.with_contributor_ip(parse_text(node)),
                b"username" => self.revision.with_contributor_name(parse_text(node)),
                _ => self.revision,
            },
            _ => self.revision,
        };

        ParserState::InsideRevision(self)
    }

    fn extract(&self) -> Option<RevisionInfo> {
        match self.depth_in {
            0 => Some(RevisionInfo::from(self.page.clone(), self.revision.clone())),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum ParserState {
    Outside(ParserStateOutsideVariant),
    InsidePage(ParserStateInsidePageVariant),
    InsideRevision(ParserStateInsideRevisionVariant),
}

impl ParserState {
    fn new() -> Self {
        Self::Outside(ParserStateOutsideVariant::new(0))
    }

    fn forward(self, node: BytesStart) -> Self {
        match self {
            Self::Outside(variant) => variant.forward(node),
            Self::InsidePage(variant) => variant.forward(node),
            Self::InsideRevision(variant) => variant.forward(node),
        }
    }

    fn backward(self, node: BytesEnd) -> Self {
        match self {
            Self::Outside(variant) => variant.backward(),
            Self::InsidePage(variant) => variant.backward(),
            Self::InsideRevision(variant) => variant.backward(node),
        }
    }

    fn process(self, node: BytesText) -> Self {
        match self {
            Self::Outside(_) => self,
            Self::InsidePage(variant) => variant.process(node),
            Self::InsideRevision(variant) => variant.process(node),
        }
    }

    fn extract(&self) -> Option<RevisionInfo> {
        match self {
            Self::Outside(_) => None,
            Self::InsidePage(_) => None,
            Self::InsideRevision(variant) => variant.extract(),
        }
    }
}

fn parse_text(node: BytesText) -> String {
    match node.into_inner() {
        Cow::Borrowed(value) => String::from(std::str::from_utf8(value).unwrap()),
        Cow::Owned(value) => String::from_utf8(value).unwrap(),
    }
}

struct RingBuffer<'a> {
    reader: Box<dyn Read + 'a>,
    data: Vec<u8>,
    left: usize,
    right: usize,
    length: usize,
}

impl<'a> RingBuffer<'a> {
    fn from(reader: Box<dyn Read + 'a>, capacity: usize) -> Self {
        let mut data = Vec::with_capacity(capacity);
        data.resize(capacity, 0);

        Self {
            reader: reader,
            data: data,
            left: 0,
            right: 0,
            length: 0,
        }
    }

    fn write_slice(&mut self) -> (&mut Box<dyn Read + 'a>, &mut [u8]) {
        if self.length == self.data.len() {
            return (&mut self.reader, &mut self.data[0..0]);
        }

        if self.left <= self.right {
            return (&mut self.reader, &mut self.data[self.right..]);
        } else {
            return (&mut self.reader, &mut self.data[self.right..self.left]);
        }
    }

    fn write_bytes(&mut self, amt: usize) {
        self.length = self.length + amt;
        self.right = (self.right + amt) % self.data.len();
    }

    fn read_slice(&self) -> &[u8] {
        if self.length == 0 {
            return &self.data[0..0];
        }

        if self.left < self.right {
            return &self.data[self.left..self.right];
        } else {
            return &self.data[self.left..];
        };
    }

    fn read_bytes(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.length == 0 {
            return Ok(0);
        }

        let chunk = if self.left < self.right {
            &self.data[self.left..self.right]
        } else {
            &self.data[self.left..]
        };

        let available = std::cmp::min(buf.len(), chunk.len());
        buf[..available].copy_from_slice(&chunk[..available]);

        self.length = self.length - available;
        self.left = (self.left + available) % self.data.len();

        Ok(available)
    }
}

impl<'a> Read for RingBuffer<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (reader, write_slice) = self.write_slice();
        let count = reader.read(write_slice)?;

        self.write_bytes(count);
        self.read_bytes(buf)
    }
}

impl<'a> BufRead for RingBuffer<'a> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let (reader, write_slice) = self.write_slice();
        let count = reader.read(write_slice)?;

        self.write_bytes(count);
        Ok(self.read_slice())
    }

    fn consume(&mut self, amt: usize) {
        self.length = self.length - amt;
        self.left = (self.left + amt) % self.data.len();
    }
}

fn process_file(input: &str, output: &str) {
    let mut stream = SevenZReader::open(input, Password::empty()).unwrap();
    let mut writer = File::create(output).unwrap();

    stream.for_each_entries(|_, reader| {
        let boxed = Box::new(reader);
        let cursor = RingBuffer::from(boxed, 10485760);

        let mut xml = Reader::from_reader(cursor);
        let mut buffer = Vec::with_capacity(10485760);
        let mut state = ParserState::new();

        loop {
            state = match xml.read_event_into(&mut buffer) {
                Err(error) => panic!("{}", error),
                Ok(Event::Eof) => break,
                Ok(Event::Start(node)) => state.forward(node),
                Ok(Event::End(node)) => {
                    if let Some(revision) = state.extract() {
                        let json = revision.as_json();

                        writer.write_all(json.as_bytes()).unwrap();
                        writer.write_all(b"\n").unwrap();
                    }

                    state.backward(node)
                }
                Ok(Event::Text(node)) => state.process(node),
                Ok(Event::Empty(_node)) => state,
                Ok(_node) => state,
            };

            buffer.clear();
        }

        Ok(true)
    }).unwrap();

    writer.flush().unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut eligible = HashSet::new();
    let (task_sender, task_receiver) = mpsc::unbounded_channel();

    let mut tasks = Vec::new();
    let receiver = Arc::new(Mutex::new(task_receiver));

    for entry in std::fs::read_dir("d:\\wikipedia").unwrap() {
        if let Ok(entry) = entry {
            if let Some(filename) = entry.path().to_str() {
                if filename.ends_with(".7z") {
                    eligible.insert(String::from(filename.trim_end_matches(".7z")));
                }
            }
        }
    }

    for entry in std::fs::read_dir("d:\\wikipedia").unwrap() {
        if let Ok(entry) = entry {
            if let Some(filename) = entry.path().to_str() {
                if filename.ends_with(".json") {
                    eligible.remove(filename.trim_end_matches(".json"));
                }
            }
        }
    }

    for entry in std::fs::read_dir("d:\\wikipedia").unwrap() {
        if let Ok(entry) = entry {
            if let Some(filename) = entry.path().to_str() {
                if filename.ends_with(".json.tmp") {
                    fs::remove_file(filename).unwrap();
                }
            }
        }
    }

    for _ in 0..12 {
        let receiver = Arc::clone(&receiver);
        let task = tokio::spawn(async move {
            while let Some(filename) = {
                let mut receiver = receiver.lock().await;
                receiver.recv().await
            } {
                let input = format!("{}.7z", filename);
                let output = format!("{}.json.tmp", filename);

                println!("Processing {0}", input);
                process_file(&input, &output);
                fs::rename(&output, format!("{}.json", filename)).unwrap();
                println!("Processed {0}", output);
            }
        });

        tasks.push(task);
    };

    for entry in eligible {
        task_sender.send(entry).unwrap();
    }

    drop(task_sender);

    for task in tasks {
        task.await.unwrap();
    }

    Ok(())
}
