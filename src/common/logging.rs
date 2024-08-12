// Tunnel

macro_rules! log_tunnel_created {
    ($peer:expr, $proxy:expr, $endpoint:expr) => {
        info!(
            "\n\n\tTunnel has been CREATED: \
          \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}]\n",
            $peer, $proxy, $endpoint
        );
    };
}

macro_rules! log_tunnel_closed {
    ($peer:expr, $proxy:expr, $endpoint:expr, $l2r:expr, $r2l:expr) => {
        info!(
            "\n\n\tTunnel has been CLOSED: \
          \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}] \
          \n\t\ttransmitted: L->R {}, R->L {}\n",
            $peer,
            $proxy,
            $endpoint,
            human_bytes($l2r as f64),
            human_bytes($r2l as f64)
        );
    };
}

macro_rules! log_tunnel_closed_with_error {
    ($peer:expr, $proxy:expr, $endpoint:expr, $err:expr) => {
        error!(
            "\n\n\tTunnel has been CLOSED with ERROR: \
          \n\t\tsource [{}] <--L--> lurk [{}] <--R--> destination [{}] \
          \n\t\terror: '{}'\n",
            $peer, $proxy, $endpoint, $err
        );
    };
}

pub(crate) use log_tunnel_closed;
pub(crate) use log_tunnel_closed_with_error;
pub(crate) use log_tunnel_created;

// 'Request' error handling

macro_rules! log_request_handling_error {
    ($conn:expr, $err:expr, $req:expr, $resp:expr) => {
        error!(
            "\n\n\tError occured during REQUEST handling: \
            \n\t\tpeer: '{}' \
            \n\t\treason: '{}' \
            \n\t\trequest : '{:?}' \
            \n\t\tresponse: '{:?}' \
            \n",
            $conn.peer_addr(),
            $err,
            $req,
            $resp
        )
    };
}

// TCP

macro_rules! log_tcp_closed_conn_with_error {
    ($conn_addr:expr, $conn_label:expr, $err:expr) => {
        error!(
            "\n\n\tTCP {} connection has been CLOSED with ERROR: \
            \n\t\tpeer: '{}' \
            \n\t\treason: '{}' \
            \n",
            $conn_label, $conn_addr, $err
        )
    };
}

macro_rules! log_tcp_closed_conn {
    ($conn_addr:expr, $conn_label:expr) => {
        info!(
            "\n\n\tTCP {} connection has been CLOSED: \
            \n\t\tpeer: '{}' \
            \n",
            $conn_label, $conn_addr,
        )
    };
}

macro_rules! log_tcp_established_conn {
    ($conn_addr:expr, $conn_label:expr) => {
        info!(
            "\n\n\tTCP connection with {} label has been OPENED: \
            \n\t\tpeer: '{}' \
            \n",
            $conn_label, $conn_addr,
        )
    };
}

macro_rules! log_tcp_acception_error {
    ($err:expr) => {
        warn!(
            "\n\n\tTCP connection was NOT ACCEPTED: \
            \n\t\treason: '{}' \
            \n",
            $err
        )
    };
}

pub(crate) use log_tcp_acception_error;
pub(crate) use log_tcp_closed_conn;
pub(crate) use log_tcp_closed_conn_with_error;
pub(crate) use log_tcp_established_conn;

pub(crate) use log_request_handling_error;
