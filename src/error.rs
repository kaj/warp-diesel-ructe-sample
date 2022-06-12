use crate::templates::{self, RenderError, RenderRucte};
use warp::http::{response::Builder, StatusCode};
use warp::reply::{Reply, Response};

#[derive(Debug)]
pub enum Error {
    NotFound,
    InternalError,
}

impl Reply for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => Builder::new()
                .status(StatusCode::NOT_FOUND)
                .html(|o| {
                    templates::error(
                        o,
                        StatusCode::NOT_FOUND,
                        "The resource you requested could not be located.",
                    )
                })
                .unwrap(),
            Error::InternalError => {
                let code = StatusCode::INTERNAL_SERVER_ERROR;
                Builder::new()
                    .status(code)
                    .html(|o| {
                        templates::error(o, code, "Something went wrong.")
                    })
                    .unwrap()
            }
        }
    }
}
impl From<RenderError> for Error {
    fn from(_: RenderError) -> Self {
        Error::InternalError
    }
}
