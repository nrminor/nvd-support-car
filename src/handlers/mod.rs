pub mod dummy;
pub mod gottcha2;
pub mod health;
pub mod stast;

pub use dummy::ingest_dummy;
pub use gottcha2::ingest_gottcha2;
pub use health::healthz;
pub use stast::ingest_stast;
