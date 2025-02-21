use message::message::Message;

mod message;
mod encode;

fn main() {
    println!("Hello, world!");

    let message = Message::try_from("CQ N0YPR/R DM42").unwrap();
    println!("Message: {}", message);

    println!("Message bits: {:b}", message.bits());

    println!("Checksum: {:b}", message.checksum());
}
