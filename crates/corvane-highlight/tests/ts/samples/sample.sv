// Parameterised synchronous FIFO with a concurrent assertion.
`timescale 1ns / 1ps
`define ASSERT_ON

package fifo_pkg;
  typedef enum logic [1:0] { EMPTY, PARTIAL, FULL } level_e;
endpackage
module fifo #(parameter int WIDTH = 8, parameter int DEPTH = 16) (
  input  logic             clk, rst_n, push, pop,
  input  logic [WIDTH-1:0] din,
  output logic [WIDTH-1:0] dout,
  output fifo_pkg::level_e level
);
  import fifo_pkg::*;
  localparam int AW = $clog2(DEPTH);

  logic [WIDTH-1:0] mem [DEPTH];
  logic [AW:0]      count;
  logic [AW-1:0]    wr_ptr, rd_ptr;
  wire do_push = push && count != DEPTH;
  wire do_pop  = pop && count != 0;
  always_ff @(posedge clk or negedge rst_n) begin
    if (!rst_n) begin
      {wr_ptr, rd_ptr, count} <= '0;
    end else begin
      if (do_push) begin mem[wr_ptr] <= din; wr_ptr <= wr_ptr + 1'b1; end
      if (do_pop) rd_ptr <= rd_ptr + 1'b1;
      count <= count + do_push - do_pop;
    end
  end

  assign dout = mem[rd_ptr];
  always_comb begin
    unique case (count)
      0:       level = EMPTY;
      DEPTH:   level = FULL;
      default: level = PARTIAL;
    endcase
  end

`ifdef ASSERT_ON
  no_overflow: assert property (@(posedge clk) disable iff (!rst_n) count <= DEPTH)
    else $error("FIFO overflow: count=%0d", count);
`endif
endmodule
