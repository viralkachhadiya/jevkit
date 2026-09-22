//! Live call. Run with: TYPESAFE_API_KEY=... cargo run --example live
use jevkit::{Client, Request};

jevkit::choice! {
    /// Which team should handle this?
    pub enum Department {
        /// Payments, invoicing, refunds
        Billing,
        /// Bugs, outages, integrations
        Technical,
        /// Pricing, upgrades, new accounts
        Sales,
    }
}

jevkit::score! {
    /// How frustrated is the customer?
    pub enum Frustration {
        Calm,
        Frustrated,
        Angry = "Very angry",
    }
}

jevkit::noul! {
    /// Does this convey urgency?
    pub enum Urgent {
        /// Explicitly time-sensitive
        yes Yes,
        /// No urgency expressed
        no No,
    }
}

fn main() -> Result<(), jevkit::Error> {
    let state = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Help! My payouts have been failing for 3 days.".into());

    let client = Client::from_env()?;
    let response = client.send(
        &Request::new(state)
            .choice::<Department>()
            .score::<Frustration>()
            .noul::<Urgent>(),
    )?;

    let dept = response.choice::<Department>()?;
    println!("model:       {}", response.model);
    println!(
        "department:  {:?} ({:.0}% confident)",
        dept.value,
        dept.confidence * 100.0
    );
    for d in dept.ranked.iter() {
        println!("  {:>10?} {:.2}", d.value, d.confidence);
    }
    let mood = response.score::<Frustration>()?;
    println!(
        "frustration: {:?} (score {:.2}, normalized {:.2})",
        mood.value, mood.score, mood.normalized
    );
    println!("urgent:      {:?}", response.noul::<Urgent>()?.value);
    println!("usage:       {:?}", response.usage);
    Ok(())
}
