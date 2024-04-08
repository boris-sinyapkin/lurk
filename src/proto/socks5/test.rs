use crate::{
    common::{
        error::{InvalidValue, LurkError, Unsupported},
        net::ipv4_socket_address,
        LurkAuthMethod,
    },
    io::{LurkRequest, LurkResponse},
    proto::socks5::{
        consts::*,
        request::{HandshakeRequest, RelayRequest},
        response::{HandshakeResponse, RelayResponse},
        Address, Command, ReplyStatus,
    },
};
use anyhow::anyhow;
use std::{
    collections::HashSet,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

macro_rules! assert_lurk_err {
    ($expected:expr, $actual:expr) => {
        assert_eq!($expected, $actual.downcast::<LurkError>().expect("Lurk error type expected"))
    };
}

macro_rules! bail_unless_expected_lurk_err {
    ($expected_lurk_err:expr, $result:expr) => {
        match $result {
            Err(err) => assert_lurk_err!($expected_lurk_err, err),
            Ok(ok) => panic!("Should fail with error, instead returned {:#?}", ok),
        }
    };
}

#[tokio::test]
async fn rw_handshake_messages() {
    let mut read_stream = tokio_test::io::Builder::new()
        .read(&[
            SOCKS5_VERSION,
            3,
            auth::SOCKS5_AUTH_METHOD_PASSWORD,
            auth::SOCKS5_AUTH_METHOD_GSSAPI,
            auth::SOCKS5_AUTH_METHOD_NONE,
        ])
        .read(&[SOCKS5_VERSION, 1, auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE])
        .build();

    let request = HandshakeRequest::read_from(&mut read_stream)
        .await
        .expect("Handshale request should be parsed");

    assert_eq!(
        &HashSet::from([LurkAuthMethod::Password, LurkAuthMethod::GssAPI, LurkAuthMethod::None]),
        request.auth_methods(),
        "Handshake request parsed incorrectly"
    );

    bail_unless_expected_lurk_err!(
        LurkError::DataError(InvalidValue::AuthMethod(auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE)),
        HandshakeRequest::read_from(&mut read_stream).await
    );

    let mut write_stream = tokio_test::io::Builder::new()
        .write(&[SOCKS5_VERSION, auth::SOCKS5_AUTH_METHOD_GSSAPI])
        .write(&[SOCKS5_VERSION, auth::SOCKS5_AUTH_METHOD_NOT_ACCEPTABLE])
        .build();

    HandshakeResponse::builder()
        .with_auth_method(LurkAuthMethod::GssAPI)
        .build()
        .write_to(&mut write_stream)
        .await
        .expect("Handshake response with defined method should be written");

    HandshakeResponse::builder()
        .with_no_acceptable_method()
        .build()
        .write_to(&mut write_stream)
        .await
        .expect("Handshake response with NoAcceptableMethod should be written");
}

#[tokio::test]
#[rustfmt::skip]
async fn rw_relay_messages() {
    let mut read_stream = tokio_test::io::Builder::new()
        .read(&[
            SOCKS5_VERSION,
            command::SOCKS5_CMD_CONNECT,
            0x00,
            address::SOCKS5_ADDR_TYPE_IPV4,
            127, 0, 0, 1, 10, 10,
        ])
        .read(&[SOCKS5_VERSION, 0xff, 0x00]) // Incorrect SOCKS5 command
        .build();

    let request = RelayRequest::read_from(&mut read_stream)
        .await
        .expect("Relay request should be parsed");

    assert_eq!(Command::Connect, request.command());
    assert_eq!(
        &ipv4_socket_address!(Ipv4Addr::new(127, 0, 0, 1), 2570),
        request.endpoint_address(),
        "Relay request parsed incorrectly"
    );

    bail_unless_expected_lurk_err!(
        LurkError::DataError(InvalidValue::SocksCommand(0xff)),
        RelayRequest::read_from(&mut read_stream).await
    );

    let mut write_stream = tokio_test::io::Builder::new()
        .write(&[
            SOCKS5_VERSION,
            reply::SOCKS5_REPLY_SUCCEEDED,
            0x00,
            address::SOCKS5_ADDR_TYPE_IPV4,
            127, 0, 0, 1, 0, 11,
        ])
        .build();

    let response = RelayResponse::builder()
        .with_success()
        .with_bound_address("127.0.0.1:11".parse().unwrap())
        .build();

    response.write_to(&mut write_stream).await.expect("Relay response should be written");
}

#[tokio::test]
#[rustfmt::skip]
async fn rw_address() {
    let mut moked_stream = tokio_test::io::Builder::new()
        .read(&[address::SOCKS5_ADDR_TYPE_IPV4, 127, 0, 0, 1, 10, 10]) // correct IPv4
        .read(&[0xff]) // invalid address type
        .build();

    let addr = Address::read_from(&mut moked_stream).await.expect("Parsed IPv4 address");
    assert_eq!(addr, ipv4_socket_address!(Ipv4Addr::new(127, 0, 0, 1), 2570));

    bail_unless_expected_lurk_err!(
        LurkError::DataError(InvalidValue::AddressType(0xff)),
        Address::read_from(&mut moked_stream).await
    );

    let addr_to_write = ipv4_socket_address!(Ipv4Addr::new(127, 0, 0, 1), 2570);
    let mut written_address = vec![];
    addr_to_write.write_to(&mut written_address);
    assert_eq!(vec![address::SOCKS5_ADDR_TYPE_IPV4, 127, 0, 0, 1, 10, 10], written_address);
}

#[test]
#[rustfmt::skip]
fn error_to_relay_status_cast() {
    let dummy_invalid_value_err = InvalidValue::AuthMethod(0xff);
    let dummy_utf8_err = String::from_utf8(vec![0xF1]).unwrap_err();

    assert_eq!(ReplyStatus::CommandNotSupported,     anyhow!(LurkError::Unsupported(Unsupported::Socks5Command(Command::Bind))).into());
    assert_eq!(ReplyStatus::AddressTypeNotSupported, anyhow!(LurkError::Unsupported(Unsupported::IPv6Address)).into());
    assert_eq!(ReplyStatus::GeneralFailure,          anyhow!(LurkError::DataError(dummy_invalid_value_err)).into());
    assert_eq!(ReplyStatus::GeneralFailure,          anyhow!(LurkError::DomainNameDecodingFailed(dummy_utf8_err)).into());
    assert_eq!(ReplyStatus::ConnectionRefused,       anyhow!(io::Error::from(io::ErrorKind::ConnectionRefused)).into());
    assert_eq!(ReplyStatus::HostUnreachable,         anyhow!(io::Error::from(io::ErrorKind::ConnectionAborted)).into());
    assert_eq!(ReplyStatus::GeneralFailure,          anyhow!(io::Error::from(io::ErrorKind::NotFound)).into());
}
