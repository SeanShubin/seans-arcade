// Error Handling with Result/Option - No exceptions in Rust
//
// Rust has no try/catch. Instead, functions that can fail return Result<T, E>
// or Option<T>. The compiler forces you to handle the error case.
//
// - Option<T> = Some(value) or None (value might not exist)
// - Result<T, E> = Ok(value) or Err(error) (operation might fail)
//
// Both are just enums. You already know how to destructure them with match.
// This file covers the ergonomic tools built on top of that.

use std::num::ParseIntError;

fn main() {
    option_basics();
    result_basics();
    question_mark_operator();
    combinators();
}

// ── OPTION ──────────────────────────────────────────────────────────
// Use when a value might not exist. Replaces null/undefined.

fn find_item(inventory: &[&str], name: &str) -> Option<usize> {
    inventory.iter().position(|&item| item == name)
}

fn option_basics() {
    let inventory = vec!["Sword", "Shield", "Potion"];

    // match - explicit handling of both cases
    match find_item(&inventory, "Shield") {
        Some(index) => println!("[option] Shield found at index {index}"),
        None => println!("[option] Shield not found"),
    }

    // if let - when you only care about the Some case
    if let Some(index) = find_item(&inventory, "Potion") {
        println!("[option] Potion at index {index}");
    }

    // unwrap_or - provide a default
    let index = find_item(&inventory, "Bow").unwrap_or(999);
    println!("[option] Bow index (with default): {index}");

    // map - transform the inner value if it exists
    let message = find_item(&inventory, "Sword")
        .map(|i| format!("Sword is in slot {i}"))
        .unwrap_or("No sword".to_string());
    println!("[option] {message}");

    // unwrap - crashes if None. Use only when you're certain it's Some.
    // let _risky = find_item(&inventory, "Missing").unwrap(); // would panic!
}

// ── RESULT ──────────────────────────────────────────────────────────
// Use when an operation can fail. Replaces throwing exceptions.

fn parse_damage(input: &str) -> Result<i32, ParseIntError> {
    input.parse::<i32>() // returns Result<i32, ParseIntError>
}

fn result_basics() {
    // match - handle success and error
    match parse_damage("42") {
        Ok(damage) => println!("[result] parsed damage: {damage}"),
        Err(e) => println!("[result] parse error: {e}"),
    }

    match parse_damage("not_a_number") {
        Ok(damage) => println!("[result] parsed: {damage}"),
        Err(e) => println!("[result] expected error: {e}"),
    }

    // unwrap_or - default on error
    let damage = parse_damage("invalid").unwrap_or(0);
    println!("[result] with fallback: {damage}");

    // is_ok / is_err - check without consuming
    println!("[result] '100' valid? {}", parse_damage("100").is_ok());
    println!("[result] 'abc' valid? {}", parse_damage("abc").is_ok());
}

// ── THE ? OPERATOR ──────────────────────────────────────────────────
// Propagates errors up to the caller. Replaces try/catch chains.
// If the Result is Ok, unwrap and continue. If Err, return early with the error.

fn parse_attack_command(input: &str) -> Result<(String, i32), String> {
    // Split "fireball 25" into target and damage
    let parts: Vec<&str> = input.split_whitespace().collect();

    let target = parts.get(0)
        .ok_or("missing target".to_string())?;  // Option -> Result, then ?

    let damage_str = parts.get(1)
        .ok_or("missing damage".to_string())?;

    let damage: i32 = damage_str.parse()
        .map_err(|e: ParseIntError| format!("bad damage: {e}"))?;  // convert error type, then ?

    Ok((target.to_string(), damage))
}

fn question_mark_operator() {
    let commands = vec!["fireball 25", "icebolt", "lightning abc", "heal 50"];

    for cmd in commands {
        match parse_attack_command(cmd) {
            Ok((target, damage)) => {
                println!("[?] '{cmd}' -> target={target}, damage={damage}");
            }
            Err(e) => {
                println!("[?] '{cmd}' -> error: {e}");
            }
        }
    }
}

// ── COMBINATORS ─────────────────────────────────────────────────────
// Chain operations on Option/Result without nested match blocks.
// Similar to .map/.flatMap on Optional/Stream in Java.

fn combinators() {
    let inventory = vec!["Sword:25", "Shield:15", "Potion:0"];

    // and_then (flatMap) - chain operations that each might fail
    for entry in &inventory {
        let damage = parse_item_damage(entry);
        println!("[combo] {entry} -> damage: {damage:?}");
    }

    // filter - keep Some only if predicate is true
    let high_damage = parse_item_damage("Sword:25")
        .filter(|&d| d > 20);
    println!("[combo] high damage filter: {high_damage:?}");

    let low_damage = parse_item_damage("Potion:0")
        .filter(|&d| d > 20);
    println!("[combo] low damage filter: {low_damage:?}");
}

fn parse_item_damage(entry: &str) -> Option<i32> {
    entry
        .split(':')          // split "Sword:25"
        .nth(1)              // get "25" (Option<&str>)
        .and_then(|s| s.parse().ok())  // parse to i32, convert Result to Option
}
