use actix_wamp;
use failure::Fail;
use golem_rpc_api;
use std::io;
use std::time;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "{}", _0)]
    GolemApiError(#[cause] golem_rpc_api::Error),
    #[fail(display = "{}", _0)]
    IoError(#[cause] io::Error),
    #[fail(display = "{}", _0)]
    SystemTimeError(#[cause] time::SystemTimeError),
    #[fail(display = "{}", _0)]
    ActixWampError(#[cause] actix_wamp::Error),
    #[fail(display = "{}", _0)]
    HoundError(#[cause] hound::Error),
    #[fail(display = "{}", _0)]
    Other(String),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<golem_rpc_api::Error> for Error {
    fn from(err: golem_rpc_api::Error) -> Self {
        Error::GolemApiError(err)
    }
}

impl From<time::SystemTimeError> for Error {
    fn from(err: time::SystemTimeError) -> Self {
        Error::SystemTimeError(err)
    }
}

impl From<actix_wamp::Error> for Error {
    fn from(err: actix_wamp::Error) -> Self {
        Error::ActixWampError(err)
    }
}

impl From<hound::Error> for Error {
    fn from(err: hound::Error) -> Self {
        Error::HoundError(err)
    }
}
