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

// Error handling

macro_rules! log_request_handling_error {
    ($peer:expr, $err:expr, $req:expr, $resp:expr) => {
        error!(
            "\n\n\tError occured during REQUEST handling: \
            \n\t\tpeer: '{}' \
            \n\t\treason: '{}' \
            \n\t\trequest : '{:?}' \
            \n\t\tresponse: '{:?}' \
            \n",
            $peer, $err, $req, $resp
        )
    };
}

macro_rules! log_closed_tcp_conn_with_error {
    ($peer:expr, $err:expr) => {
        error!(
            "\n\n\tTCP connection with {} has been CLOSED with ERROR: \
            \n\t\treason: '{}' \
            \n",
            $peer, $err
        )
    };
}

macro_rules! log_closed_tcp_conn {
    ($peer:expr) => {
        info!("TCP connection with {} has been CLOSED", $peer)
    };
}

macro_rules! log_opened_tcp_conn {
    ($addr:expr) => {
        info!("TCP connection with {} has been OPENED", $addr)
    };
}

pub(crate) use log_opened_tcp_conn;
pub(crate) use log_closed_tcp_conn;
pub(crate) use log_closed_tcp_conn_with_error;
pub(crate) use log_request_handling_error;
