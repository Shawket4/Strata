//! `MsgPack<T>` as a response.

use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{HttpRequest, HttpResponse, Responder, ResponseError};
use serde::Serialize;

use super::{MSGPACK, MsgPack, Problem, ProblemType, encode};

impl<T: Serialize> Responder for MsgPack<T> {
    type Body = BoxBody;

    /// `200 OK` with `Content-Type: application/msgpack`. Use
    /// `.customize().with_status(StatusCode::CREATED)` for other success statuses.
    fn respond_to(self, _req: &HttpRequest) -> HttpResponse {
        match encode(&self.0) {
            Ok(bytes) => HttpResponse::Ok()
                .insert_header(ContentType(msgpack_mime()))
                .body(bytes),
            Err(err) => {
                tracing::error!(error = %err, "failed to encode a MessagePack response");
                Problem::new(ProblemType::Internal).error_response()
            }
        }
    }
}

fn msgpack_mime() -> mime::Mime {
    // Invariant: MSGPACK is a valid media type literal.
    MSGPACK.parse().unwrap_or(mime::APPLICATION_OCTET_STREAM)
}
