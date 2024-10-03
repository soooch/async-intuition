//! Each of the below modules demonstrate a simple async operation implemented
//! in two ways. Each module is structured as follows (where `operation` is the
//! module name):
//!
//! ```
//! pub mod operation {
//!     //! The operation available in the auto and manual modules are
//!     //! functionally identical. Additionally, both should compile down to
//!     //! essentially the same code.
//!
//!     pub mod auto {
//!         pub async fn operation() {
//!            // procedure implemented via Rust async-await syntax.
//!         }
//!     }
//!
//!     pub mod manual {
//!         pub async fn operation() {
//!             // procuedure implemented as a struct which implements
//!             // [`std::future::Future`].
//!         }
//!     }
//! }
//! ```

pub mod a_then_b;
pub mod until_equals;
