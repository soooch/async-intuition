//! If you ever need to work with the [`Future`] trait directly, you'll notice
//! [`std::pin::Pin`]. To understand [`Pin`], we'll first cause the error it's
//! designed to prevent.
//!
//! ```
//! struct Dangerous<T> {
//!     val: T,
//!     ptr: *const T,
//! }
//!
//! fn a_bad_idea<T>(val: T) {
//!     let mut initial = Dangerous { val, ptr: std::ptr::null() };
//!     initial.ptr = std::ptr::from_ref(&initial.val);
//!     let defined = unsafe { &*initial.ptr };
//!     let moved = Box::new(initial);
//!     let undefined = unsafe { &*moved.ptr };
//! }
//! ```
//!
//! Did you catch what went wrong there? We constructed a struct with a pointer
//! pointing inside itself, then moved that struct. To make it more concrete,
//! say we have a `Dangerous<usize>` laid out in memory as follows:
//!
//! ```txt
//! location: field (value)
//! ----------------------
//! 0x20..0x28: val (0x03)
//! 0x28..0x30: ptr (0x20)
//! ```
//!
//! We move that struct to another location in memory resulting in:
//!
//! ```txt
//! location: field (value)
//! ----------------------
//! 0x50..0x58: val (0x03)
//! 0x58..0x60: ptr (0x20)
//! ```
//!
//! If our "move" was a `move` in the Rust sense, then the previous location is
//! now invalid. Unfortunately, our pointer `ptr` still refers to that previous
//! location (the stored value is still 0x20, where `val` used to be). This
//! means writing to or reading from `ptr` is now undefined behavior.
//!
//! But who in the world would want a self-referential struct? What real world
//! need does this fulfill? Let's circle back to [`Future`]s:
//!
//! ```
//! # use std::fmt::Display;
//! #
//! # use futures_lite::future::yield_now;
//! #
//! # fn choose() -> bool {
//! #     true
//! # }
//! #
//! async fn self_ref<T: Display>(a: T, b: T) {
//!     let choice = if choose() { &a } else { &b };
//!     yield_now().await;
//!     println!("{choice}");
//! }
//! ```
//!
//! Because `a`, `b`, and `choice` are held across an `await` point, each must
//! be representable as fields on the [`Future`] which `self_ref` compiles to.
//! the field for `choice` will hold a pointer which references either the
//! field for `a` or `b`. As such, the [`Future`] constructed by `self_ref`
//! must not be moved.
//!
//! So how does [`Pin`] help us? Essentially, when a pointer type is wrapped in
//! [`Pin`], the pointed to value may never again be moved (unless that type
//! implements [`Unpin`] which is a promise that the type is safely movable).
//! This is accomplished through a few tricks:
//!
//! 1. once a pinning ref [`Pin<Ptr<T>>`][`Pin`] is constructed from a value T,
//!    the only way to ever again access that value is through the pinning ref
//!    or the [`Drop`] impl of `T`.
//!
//! 2. there is no way to [`std::mem::replace`] or [`std::mem::take`] from a
//!    pinning ref since [`Pin`] does not impl [`std::ops::DerefMut`].
//!
//! 3. the only allowed mutating operation to [`Pin<Ptr>`][`Pin`] is
//!    [`Pin::set`].
//!
//! module name stolen from <https://fasterthanli.me/articles/pin-and-suffering>
//!
//! [`Future`]: std::future::Future
//! [`Pin`]: std::pin::Pin
//! [`Pin::set`]: std::pin::Pin::set
