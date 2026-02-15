fn main() {
    let languages = vec!["Rust", "Bevy", "ECS"];

    let greeting: String = languages
        .iter()
        .map(|lang| format!("Hello, {lang}!"))
        .collect::<Vec<_>>()
        .join(" ");

    println!("{greeting}");
}
