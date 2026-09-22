//! Offline demo of specs and decoding. Run with: cargo run --example triage
use jevkit::{decode_choice, decode_noul, decode_score, Choice, Noul, Score};

jevkit::choice! {
    /// Which team should handle this ticket?
    pub enum Department {
        /// Payments, invoices and refunds
        Billing,
        /// Bugs and outages
        Technical,
        /// Anything else
        Other,
    }
}

jevkit::score! {
    /// How frustrated is the customer?
    pub enum Frustration {
        Calm,
        Annoyed,
        Furious,
    }
}

jevkit::noul! {
    /// Does the message convey urgency?
    pub enum Urgency {
        /// There is a deadline
        yes Urgent,
        /// Routine request
        no Routine,
    }
}

fn main() {
    println!("{:#?}", Department::SPEC);
    println!("{:#?}", Frustration::SPEC);
    println!("{:#?}", Urgency::SPEC);

    // Pretend these probabilities came back from Jev.
    let dept = decode_choice::<Department>(&[("Billing", 0.7), ("Technical", 0.2), ("Other", 0.1)])
        .unwrap();
    let mood = decode_score::<Frustration>(&[0.1, 0.3, 0.6]).unwrap();
    let urgent = decode_noul::<Urgency>(0.55).unwrap();

    match dept.into_best().into_confident(0.8) {
        Ok(d) => println!("route to {d:?}"),
        Err(d) => println!(
            "only {:.0}% sure it is {:?}: send to a human",
            d.confidence * 100.0,
            d.value
        ),
    }
    println!("mood: {:?} ({:.0}%)", mood.value, mood.confidence * 100.0);
    println!("urgent: {:?}", urgent.value);
}
