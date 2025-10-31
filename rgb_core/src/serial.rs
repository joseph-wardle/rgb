// Communication between two Game Boy systems happens one byte at a time. One Game Boy generates a
// clock signal internally and thus controls when the exchange happens. In SPI terms, the Game Boy
// generating the clock is called the “master” while the other one (the “slave” Game Boy) receives
// it. If it hasn’t gotten around to loading up the next data byte at the time the transfer begins,
// the last one will go out again. Alternately, if it’s ready to send the next byte but the last one
// hasn’t gone out yet, it has no choice but to wait.
//
// Before a transfer, the SB byte holds the next byte that will go out. During a transfer, it has
// a blend of the outgoing and incoming bytes. Each cycle, the leftmost bit is shifted out (and over
// the wire) and the incoming bit is shifted in from the other side:
//
// |    SB    |   7   |   6   |   5   |   4   |   3   |   2   |   1   |   0   |
// | -------- | ----- | ----- | ----- | ----- | ----- | ----- | ----- | ----- |
// | Initial  | out.7 | out.6 | out.5 | out.4 | out.3 | out.2 | out.1 | out.0 |
// | 1 shift  | out.6 | out.5 | out.4 | out.3 | out.2 | out.1 | out.0 | in.7  |
// | 2 shifts | out.5 | out.4 | out.3 | out.2 | out.1 | out.0 | in.7  | in.6  |
// | 3 shifts | out.4 | out.3 | out.2 | out.1 | out.0 | in.7  | in.6  | in.5  |
// | 4 shifts | out.3 | out.2 | out.1 | out.0 | in.7  | in.6  | in.5  | in.4  |
// | 5 shifts | out.2 | out.1 | out.0 | in.7  | in.6  | in.5  | in.4  | in.3  |
// | 6 shifts | out.1 | out.0 | in.7  | in.6  | in.5  | in.4  | in.3  | in.2  |
// | 7 shifts | out.0 | in.7  | in.6  | in.5  | in.4  | in.3  | in.2  | in.1  |
// | 8 shifts | in.7  | in.6  | in.5  | in.4  | in.3  | in.2  | in.1  | in.0  |
//
// |     |        7        |  6   5   4   3   2  |      1      |      0       |
// | --- | --------------- | ------------------- | ----------- | ------------ |
// | SC  | Transfer enable |                     | Clock speed | Clock select |

#[derive(Debug, Default)]
pub struct Serial {
    pub sb: u8, // Serial transfer data
    pub sc: u8, // Serial transfer control
    buffer: Vec<u8>,
}

impl Serial {
    pub fn output_string(&self) -> String {
        String::from_utf8_lossy(&self.buffer).to_string()
    }
}

impl Serial {
    pub fn new() -> Self {
        Serial {
            sb: 0,
            sc: 0,
            buffer: Vec::new(),
        }
    }

    pub fn write_control(&mut self, value: u8) {
        self.sc = value;
        if self.sc & 0x80 != 0 {
            self.buffer.push(self.sb);
            self.sc &= 0x7F; // transfer complete
        }
    }
}
