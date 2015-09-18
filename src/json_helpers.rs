//! Helpers to standardize decoding Algorithmia JSON responses

use error::{Error, ApiErrorResponse};
use rustc_serialize::{json, Decodable};

pub fn decode_response<T: Decodable>(res_json: &str) -> Result<T, Error> {
    match json::decode::<ApiErrorResponse>(res_json) {
        Ok(err_res) => Err(err_res.error.into()),
        Err(_) => decode(res_json),
    }
}


pub fn decode<T: Decodable>(res_json: &str) -> Result<T, Error> {
    match json::decode::<T>(res_json) {
        Ok(result) => Ok(result),
        Err(err) => Err(Error::DecoderErrorWithContext(err, res_json.into())),
    }
}


pub fn decode_to_error(res_json: &str) -> Error {
    match json::decode::<ApiErrorResponse>(res_json) {
        Ok(err_res) => err_res.error.into(),
        Err(err) => Error::DecoderErrorWithContext(err, res_json.into()),
    }
}
