// 8-bit UART transmitter with a configurable baud divider.
`timescale 1ns / 1ps
`define IDLE_BIT 1'b1

module uart_tx #(parameter CLK_HZ = 50_000_000, parameter BAUD = 115200) (
    input  wire       clk, rst, start,
    input  wire [7:0] data,
    output reg        tx,
    output wire       busy
);
    localparam DIV = CLK_HZ / BAUD;
    localparam [1:0] S_IDLE = 2'd0, S_START = 2'd1, S_DATA = 2'd2, S_STOP = 2'd3;

    reg [1:0]  state;
    reg [15:0] counter;
    reg [2:0]  bit_idx;
    reg [7:0]  shift;

    assign busy = (state != S_IDLE);

    /* One state transition per baud tick. */
    always @(posedge clk) begin
        if (rst) begin
            state <= S_IDLE;
            tx <= `IDLE_BIT;
            counter <= 16'h0000;
        end else if (counter != 0) begin
            counter <= counter - 1'b1;
        end else begin
            counter <= DIV - 1;
            case (state)
                S_IDLE:  if (start) begin shift <= data; state <= S_START; end
                S_START: begin tx <= 1'b0; bit_idx <= 3'b000; state <= S_DATA; end
                S_DATA:  begin
                    tx <= shift[bit_idx];
                    if (&bit_idx) state <= S_STOP; else bit_idx <= bit_idx + 1;
                end
                default: begin tx <= `IDLE_BIT; state <= S_IDLE; end
            endcase
        end
    end

    initial $display("uart_tx: divider = %0d", DIV);
endmodule
