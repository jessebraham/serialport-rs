#![cfg(windows)]
extern crate serialport;

#[test]
fn test_set_large_buffer() {
    const STRIDE: usize = 2;
    const WORDS: usize = 65536;
    const BYTES: usize = WORDS * STRIDE;

    let mut tx_port = serialport::new("COM12", 9600).open().unwrap();
    let mut rx_port = serialport::new("COM13", 9600).open().unwrap();

    tx_port.set_buffer_size(0, BYTES).unwrap();
    rx_port.set_buffer_size(BYTES, 0).unwrap();

    let mut rx_buf = [0_u8; BYTES];
    let mut tx_buf = [0_u8; BYTES];

    for i in 0..WORDS {
        for j in 0..STRIDE {
            tx_buf[i * STRIDE + j] = (i >> (j * 8) & 0xFF) as u8;
        }
    }

    tx_port.write_all(&tx_buf[..]).unwrap();
    rx_port.read_exact(&mut rx_buf).unwrap();

    let transmitted = tx_buf.to_vec();
    let received = rx_buf.to_vec();

    println!("Transmitted: {:?}", &transmitted[..8]);
    println!("Received: {:?}", &received[..8]);

    assert_eq!(transmitted, received);
}
