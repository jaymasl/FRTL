pub mod account;
pub mod auth_handlers;
pub mod magic_link;

pub use self::auth_handlers::{login, register, refresh_token};
pub use self::account::{change_email, change_password, delete_account, get_profile, request_delete_account, verify_delete_account};
pub use self::magic_link::{request_magic_link, verify_magic_link};