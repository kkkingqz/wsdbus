//! The fake serves exactly the interface described in `spec/`.

use wsdbus_contract::{BUS_NAME, INTERFACE, OBJECT_PATH};
use wsdbus_fake::bus::PrivateBus;
use wsdbus_fake::{Config, Fake};

/// The `<interface>` element of `INTERFACE`, without comments, annotations
/// and whitespace between tags.
fn interface(xml: &str) -> String {
    let start = xml
        .find(&format!("<interface name=\"{INTERFACE}\">"))
        .expect("interface element");
    let end =
        start + xml[start..].find("</interface>").expect("end of interface") + "</interface>".len();
    let mut text = xml[start..end].to_owned();
    while let Some(open) = text.find("<!--") {
        let close = open + text[open..].find("-->").expect("end of comment") + 3;
        text.replace_range(open..close, "");
    }
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("<annotation"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn matches_spec() {
    async_io::block_on(async {
        let bus = PrivateBus::start().unwrap();
        let _fake = Fake::serve(&bus.connect().await.unwrap(), Config::default())
            .await
            .unwrap();
        let client = bus.connect().await.unwrap();
        let reply = client
            .call_method(
                Some(BUS_NAME),
                OBJECT_PATH,
                Some("org.freedesktop.DBus.Introspectable"),
                "Introspect",
                &(),
            )
            .await
            .unwrap();
        let served: String = reply.body().deserialize().unwrap();
        let spec = include_str!("../../../spec/io.github.kkkingqz.WsDbus.Desktop1.xml");
        assert_eq!(interface(&served), interface(spec), "served XML:\n{served}");
    });
}
