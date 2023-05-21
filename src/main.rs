use std::collections::HashMap;
use std::error::Error;

use quick_xml::events::{Event, BytesStart};
use quick_xml::reader::Reader;

fn increment_counters(seen: &mut HashMap<String, usize>, key: String) {
    if let Some(value) = seen.get_mut(&key) {
        (*value) += 1;
    } else {
        seen.insert(key, 1);
    }
}

fn process_attributes(seen: &mut HashMap<String, usize>, path: &mut Vec<String>, node: BytesStart) {
    path.push("@attribute".into());

    for attribute in node.attributes() {
        path.push(format!(
            "{:?}",
            String::from_utf8(attribute.unwrap().key.0.to_vec()).unwrap()
        ));
        increment_counters(seen, path.join(" / "));
        path.pop();
    }

    path.pop();
}

fn print_results(seen: &HashMap<String, usize>, counter: &usize) {
    let mut keys: Vec<&String> = seen.keys().into_iter().collect();
    keys.sort();

    for item in keys {
        println!("{}: {:?}", item, seen.get(item).unwrap());
    }

    println!("Found not considered nodes: {}", counter);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let filename = "/home/vscode/enwiki-20230501-pages-meta-history1.xml-p1p844";
    let mut reader = Reader::from_file(filename).unwrap();

    let mut path = Vec::new();
    let mut seen = HashMap::new();

    let mut buffer = Vec::new();
    let mut counter = 0;

    loop {
        match reader.read_event_into(&mut buffer) {
            Err(error) => break println!("{}", error),
            Ok(Event::Eof) => break println!("Completed."),
            Ok(Event::Start(node)) => {
                path.push(format!("{:?}", String::from_utf8(node.name().0.to_vec()).unwrap()));
                increment_counters(&mut seen, path.join(" / "));
                process_attributes(&mut seen, &mut path, node);
            }
            Ok(Event::End(_)) => {
                path.pop();
            }
            Ok(Event::Text(_)) => {
                path.push("@text".into());
                increment_counters(&mut seen, path.join(" / "));
                path.pop();
            }
            Ok(Event::Empty(node)) => {
                path.push(format!("{:?}", String::from_utf8(node.name().0.to_vec()).unwrap()));
                increment_counters(&mut seen, path.join(" / "));
                process_attributes(&mut seen, &mut path, node);
                path.pop();
            }
            Ok(_) => counter += 1,
        }

        buffer.clear();
    }

    print_results(&seen, &counter);
    Ok(())
}
