module narrow_axi_incr (
    input wire ap_clk,
    input wire ap_rst_n,
    input wire ap_start,
    output wire ap_done,
    output wire ap_ready,
    output wire ap_idle,
    input wire [63:0] a_offset,
    output wire [63:0] m_axi_a_ARADDR,
    output wire [1:0] m_axi_a_ARBURST,
    output wire [3:0] m_axi_a_ARCACHE,
    output wire [0:0] m_axi_a_ARID,
    output wire [7:0] m_axi_a_ARLEN,
    output wire [1:0] m_axi_a_ARLOCK,
    output wire [2:0] m_axi_a_ARPROT,
    output wire [3:0] m_axi_a_ARQOS,
    input wire m_axi_a_ARREADY,
    output wire [2:0] m_axi_a_ARSIZE,
    output wire m_axi_a_ARVALID,
    input wire [63:0] m_axi_a_RDATA,
    input wire [0:0] m_axi_a_RID,
    input wire m_axi_a_RLAST,
    output wire m_axi_a_RREADY,
    input wire [1:0] m_axi_a_RRESP,
    input wire m_axi_a_RVALID,
    output wire [63:0] m_axi_a_AWADDR,
    output wire [1:0] m_axi_a_AWBURST,
    output wire [3:0] m_axi_a_AWCACHE,
    output wire [0:0] m_axi_a_AWID,
    output wire [7:0] m_axi_a_AWLEN,
    output wire [1:0] m_axi_a_AWLOCK,
    output wire [2:0] m_axi_a_AWPROT,
    output wire [3:0] m_axi_a_AWQOS,
    input wire m_axi_a_AWREADY,
    output wire [2:0] m_axi_a_AWSIZE,
    output wire m_axi_a_AWVALID,
    output reg [63:0] m_axi_a_WDATA,
    output wire m_axi_a_WLAST,
    input wire m_axi_a_WREADY,
    output reg [7:0] m_axi_a_WSTRB,
    output wire m_axi_a_WVALID,
    input wire [0:0] m_axi_a_BID,
    output wire m_axi_a_BREADY,
    input wire [1:0] m_axi_a_BRESP,
    input wire m_axi_a_BVALID
);
  localparam IDLE = 0;
  localparam READ_ADDR = 1;
  localparam READ_DATA = 2;
  localparam WRITE_ADDR = 3;
  localparam WRITE_DATA = 4;
  localparam WRITE_RESP = 5;
  localparam DONE = 6;
  reg [2:0] state;
  reg [1:0] read_beat;
  reg [1:0] write_beat;
  reg [31:0] read_value;
  reg [31:0] expected_value;

  assign ap_done = state == DONE;
  assign ap_ready = state == DONE;
  assign ap_idle = state == IDLE || state == DONE;
  assign m_axi_a_ARADDR = a_offset;
  assign m_axi_a_ARBURST = 2'b01;
  assign m_axi_a_ARCACHE = 4'd0;
  assign m_axi_a_ARID = 1'b0;
  assign m_axi_a_ARLEN = 8'd2;
  assign m_axi_a_ARLOCK = 2'd0;
  assign m_axi_a_ARPROT = 3'd0;
  assign m_axi_a_ARQOS = 4'd0;
  assign m_axi_a_ARSIZE = 3'd2;
  assign m_axi_a_ARVALID = state == READ_ADDR;
  assign m_axi_a_RREADY = state == READ_DATA;
  assign m_axi_a_AWADDR = a_offset;
  assign m_axi_a_AWBURST = 2'b01;
  assign m_axi_a_AWCACHE = 4'd0;
  assign m_axi_a_AWID = 1'b0;
  assign m_axi_a_AWLEN = 8'd2;
  assign m_axi_a_AWLOCK = 2'd0;
  assign m_axi_a_AWPROT = 3'd0;
  assign m_axi_a_AWQOS = 4'd0;
  assign m_axi_a_AWSIZE = 3'd2;
  assign m_axi_a_AWVALID = state == WRITE_ADDR;
  assign m_axi_a_WVALID = state == WRITE_DATA;
  assign m_axi_a_WLAST = write_beat == 2;
  assign m_axi_a_BREADY = state == WRITE_RESP;

  // Four-byte transfers alternate between the lower and upper bus lanes.
  always @(*) begin
    read_value = read_beat == 1 ? m_axi_a_RDATA[63:32] : m_axi_a_RDATA[31:0];
    case (read_beat)
      0: expected_value = 32'h11223344;
      1: expected_value = 32'h55667788;
      default: expected_value = 32'h99aabbcc;
    endcase
    case (write_beat)
      0: begin
        m_axi_a_WDATA = 64'hfedcba98_a1b2c3d4;
        m_axi_a_WSTRB = 8'h05;
      end
      1: begin
        m_axi_a_WDATA = 64'he5f60718_76543210;
        m_axi_a_WSTRB = 8'hf0;
      end
      default: begin
        m_axi_a_WDATA = 64'hfedcba98_293a4b5c;
        m_axi_a_WSTRB = 8'h05;
      end
    endcase
  end

  always @(posedge ap_clk) begin
    if (!ap_rst_n) begin
      state <= IDLE;
      read_beat <= 0;
      write_beat <= 0;
    end else begin
      case (state)
        IDLE: if (ap_start) state <= READ_ADDR;
        READ_ADDR: if (m_axi_a_ARREADY) state <= READ_DATA;
        READ_DATA: if (m_axi_a_RVALID) begin
          if (read_value != expected_value || m_axi_a_RLAST != (read_beat == 2) ||
              m_axi_a_RRESP != 0 || m_axi_a_RID != 0) begin
            $fatal(1, "narrow AXI read beat %0d: expected %h, got %h", read_beat,
                   expected_value, read_value);
          end
          if (read_beat == 2) state <= WRITE_ADDR;
          else read_beat <= read_beat + 1'b1;
        end
        WRITE_ADDR: if (m_axi_a_AWREADY) state <= WRITE_DATA;
        WRITE_DATA: if (m_axi_a_WREADY) begin
          if (write_beat == 2) state <= WRITE_RESP;
          else write_beat <= write_beat + 1'b1;
        end
        WRITE_RESP: if (m_axi_a_BVALID) begin
          if (m_axi_a_BID != 0 || m_axi_a_BRESP != 0) begin
            $fatal(1, "narrow AXI write response mismatch");
          end
          state <= DONE;
        end
        default: begin end
      endcase
    end
  end
endmodule
