// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Local HTTP contracts for injected CPU response processing.

use crate::api::{
    ClientIdentity, ProcessingUnavailable, ReaderSettings, ResponseExecutor, ResponseProcessor,
    ResponseTask, UexReader,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use tallyline_reference_data::api::ReferenceDataFetcher;
use tallyline_reference_data::api::model::{Endpoint, EndpointData};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Admission(mpsc::SyncSender<ResponseTask>);
impl ResponseExecutor for Admission {
    fn enqueue(&self, task: ResponseTask) -> Result<(), ProcessingUnavailable> {
        self.0.try_send(task).map_err(|_| ProcessingUnavailable)
    }
}
#[tokio::test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
async fn r_nf_3_r_api_1__envelope_and_decimal_mapping_both_use_the_injected_cpu_boundary() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).and(path("/2.0/commodities/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"status":"ok","data":[{"id":7,"name":"Recorded","code":"FIXTURE","is_available_live":1,"price_buy":"123.45","price_sell":"234.56"}]})))
        .expect(1).mount(&server).await;
    let (sender, receiver) = mpsc::sync_channel::<ResponseTask>(2);
    let executions = Arc::new(AtomicUsize::new(0));
    let recorded = Arc::clone(&executions);
    let worker = std::thread::spawn(move || {
        while let Ok(task) = receiver.recv() {
            assert!(tokio::runtime::Handle::try_current().is_err());
            recorded.fetch_add(1, Ordering::SeqCst);
            task.execute();
        }
    });
    let processor = ResponseProcessor::new(Arc::new(Admission(sender)));
    let settings = ReaderSettings::for_test_server(
        &format!("{}/2.0/", server.uri()),
        ClientIdentity::new("test", "1.2.3"),
    )
    .unwrap();
    let reader = UexReader::new_with_processor(&settings, processor).unwrap();
    let EndpointData::Commodities(items) = reader.fetch(Endpoint::Commodities).await.unwrap()
    else {
        panic!("wrong endpoint");
    };
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].average_price(
            tallyline_shared_kernel::api::model::UexEnvironment::Live,
            tallyline_shared_kernel::api::model::TradeSide::Buy
        ),
        Some("123.45".parse().unwrap())
    );
    assert_eq!(
        executions.load(Ordering::SeqCst),
        2,
        "Both JSON envelope inspection and typed decimal mapping must leave the asynchronous I/O poll"
    );
    drop(reader);
    worker.join().unwrap();
}
