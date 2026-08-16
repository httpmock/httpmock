//! Server-side state for recordings.

use std::{collections::BTreeMap, time::Duration};

use bytes::Bytes;

use crate::{
    common::data::{
        self, ActiveRecording, MockDefinition, MockServerHttpResponse, RecordingRuleConfig, RequestRequirements,
    },
    prelude::HttpMockRequest,
    server::{
        persistence::{deserialize_mock_defs_from_yaml, serialize_mock_defs_to_yaml},
        state::{
            Error,
            Error::{DataConversionError, ValidationError},
            Manager, request_matches,
        },
    },
};

#[derive(Default)]
pub(super) struct State {
    next_recording_id: usize,
    recordings: BTreeMap<usize, ActiveRecording>,
}

impl Manager {
    pub(crate) fn create_recording(&self, config: RecordingRuleConfig) -> ActiveRecording {
        let mut state = self.state.lock().unwrap();

        let rec = ActiveRecording {
            id: state.recording.next_recording_id,
            config,
            mocks: Vec::new(),
        };

        state.recording.recordings.insert(rec.id, rec.clone());

        state.recording.next_recording_id += 1;

        rec
    }

    pub(crate) fn delete_recording(&self, id: usize) -> Option<ActiveRecording> {
        let mut state = self.state.lock().unwrap();

        let result = state.recording.recordings.remove(&id);

        if result.is_some() {
            tracing::debug!("Deleting proxy rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete proxy rule with id={} (no proxy rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_recordings(&self) {
        let mut state = self.state.lock().unwrap();
        state.recording.recordings.clear();

        tracing::debug!("Deleted all recorders");
    }

    pub(crate) fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error> {
        let state = self.state.lock().unwrap();

        if let Some(rec) = state.recording.recordings.get(&id) {
            return Ok(Some(
                serialize_mock_defs_to_yaml(&rec.mocks).map_err(|err| DataConversionError(err.to_string()))?,
            ));
        }

        Ok(None)
    }

    pub(crate) fn load_mocks_from_recording(&self, recording_file_content: &str) -> Result<Vec<usize>, Error> {
        let all_static_mock_defs = deserialize_mock_defs_from_yaml(recording_file_content)
            .map_err(|err| DataConversionError(err.to_string()))?;

        if all_static_mock_defs.is_empty() {
            return Err(ValidationError(
                "no mock definitions could be found in the provided recording content".to_string(),
            ));
        }

        let mut mock_ids = Vec::with_capacity(all_static_mock_defs.len());

        for static_mock_def in all_static_mock_defs {
            let mock_def: MockDefinition = static_mock_def
                .try_into()
                .map_err(|err: data::Error| DataConversionError(err.to_string()))?;

            let active_mock = self.add_mock(mock_def, false)?;
            mock_ids.push(active_mock.id);
        }

        Ok(mock_ids)
    }

    pub(crate) fn record<
        IntoResponse: TryInto<MockServerHttpResponse, Error = impl std::fmt::Display + std::fmt::Debug + 'static>,
    >(
        &self,
        is_proxied: bool,
        time_taken: Duration,
        req: HttpMockRequest,
        res: IntoResponse,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();

        let recording_ids: Vec<usize> = state
            .recording
            .recordings
            .values()
            .filter(|rec| request_matches(&state.matchers, &req, &rec.config.request_requirements))
            .map(|r| r.id)
            .collect();

        if recording_ids.is_empty() {
            return Ok(());
        }

        let res = res.try_into().map_err(|err| DataConversionError(err.to_string()))?;

        for id in recording_ids {
            let rec = state.recording.recordings.get_mut(&id).unwrap();
            let definition = build_mock_definition(is_proxied, time_taken, &req, &res, &rec.config)?;
            rec.mocks.push(definition);
        }

        Ok(())
    }
}

fn build_mock_definition(
    is_proxied: bool,
    time_taken: Duration,
    request: &HttpMockRequest,
    response: &MockServerHttpResponse,
    config: &RecordingRuleConfig,
) -> Result<MockDefinition, Error> {
    // ************************************************************************************
    // Request
    let mut headers = Vec::with_capacity(config.record_headers.len());
    for header_name in &config.record_headers {
        let header_name_lowercase = header_name.to_lowercase();
        for (key, value) in request.headers() {
            if let Some(key) = key
                && header_name_lowercase == key.to_string().to_lowercase()
            {
                let value = value.to_str().map_err(|err| DataConversionError(err.to_string()))?;
                headers.push((header_name.to_string(), value.to_string()))
            }
        }
    }

    let request = RequestRequirements {
        /* Authority and scheme are assumed to always exist for proxies requests for the
        following reasons:

        RFC 7230 - Hypertext Transfer Protocol (HTTP/1.1): Message Syntax and Routing
        Section 5.3.2 (absolute-form):
        The section clearly states that an absolute URI (absolute-form) must be used when the
        request is made to a proxy. This inclusion of the full URI (including the scheme,
        host, and optional port) ensures that the proxy can correctly interpret the destination
        of the request without additional context.
        Exact Text from RFC 7230:
        The RFC says under Section 5.3.2:

        "absolute-form = absolute-URI"
        "When making a request to a proxy, other than a CONNECT or server-wide
        OPTIONS request (as detailed in Section 5.3.4), a client MUST send
        the target URI in absolute-form as the request-target."
        "An example absolute-form of request-line would be:
        GET http://www.example.org/pub/WWW/TheProject.html HTTP/1.1"
        */
        host: if is_proxied {
            request.uri().host().map(|h| h.to_string())
        } else {
            None
        },
        port: if is_proxied {
            request.uri().port().map(|h| h.as_u16())
        } else {
            None
        },
        scheme: if is_proxied {
            request.uri().scheme().map(|h| h.to_string())
        } else {
            None
        },
        path: Some(request.uri().path().to_string()),
        method: Some(request.method().to_string()),
        header: if !headers.is_empty() { Some(headers) } else { None },
        body: if request.body().is_empty() {
            None
        } else {
            Some(request.body().clone())
        },
        query_param: if request.query_param_length() == 0 {
            None
        } else {
            Some(request.query_params())
        },
        ..Default::default()
    };

    // ************************************************************************************
    // Response
    let mut response = response.clone();

    if config.record_response_delays {
        response.delay = Some(time_taken.as_millis() as u64)
    }

    Ok(MockDefinition { request, response })
}
