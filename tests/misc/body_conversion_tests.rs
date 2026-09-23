use httpmock::{HttpMockRequest, HttpMockResponse, server::RequestMetadata};

const BODIES: &[&[u8]] = &[b"\0boxed body\xff", b""];

fn boxed_request(body: &[u8]) -> http::Request<Box<[u8]>> {
    let mut request = http::Request::new(Box::from(body));
    request.extensions_mut().insert(RequestMetadata::new("http"));
    request
}

#[test]
fn owned_boxed_request_reuses_body_allocation() {
    for &body in BODIES {
        let request = boxed_request(body);
        let allocation = request.body().as_ptr();

        let converted: HttpMockRequest = request.into();

        assert_eq!(converted.body().as_ref(), body);
        if !body.is_empty() {
            assert_eq!(converted.body().as_ref().as_ptr(), allocation);
        }
    }
}

#[test]
fn owned_boxed_response_reuses_body_allocation() {
    for &body in BODIES {
        let response = http::Response::new(Box::<[u8]>::from(body));
        let allocation = response.body().as_ptr();

        let converted: HttpMockResponse = response.into();
        let converted_body = converted.body();

        assert_eq!(converted_body, body);
        if !body.is_empty() {
            assert_eq!(converted_body.as_ptr(), allocation);
        }
    }
}

#[test]
fn borrowed_boxed_request_keeps_an_independent_body_copy() {
    for &body in BODIES {
        let mut request = boxed_request(body);
        let allocation = request.body().as_ptr();

        let converted = HttpMockRequest::try_from(&request).unwrap();

        assert_eq!(request.body().as_ref(), body);
        assert_eq!(request.body().as_ptr(), allocation);
        request.body_mut().fill(42);
        drop(request);
        assert_eq!(converted.body().as_ref(), body);
    }
}

#[test]
fn cloned_boxed_response_keeps_an_independent_body_copy() {
    for &body in BODIES {
        let mut response = http::Response::new(Box::<[u8]>::from(body));
        let allocation = response.body().as_ptr();

        let converted = HttpMockResponse::from(response.clone());

        assert_eq!(response.body().as_ref(), body);
        assert_eq!(response.body().as_ptr(), allocation);
        response.body_mut().fill(42);
        drop(response);
        assert_eq!(converted.body(), body);
    }
}
