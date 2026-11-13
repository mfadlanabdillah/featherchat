//! Shared pieces of Feather: the message hub, the network client and the
//! phone web page. The desktop window and the headless server both use this
//! crate, so the hub has no drawing code in it at all.

pub mod net;
