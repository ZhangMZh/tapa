module axis_handshake (
    input wire ap_clk,
    input wire ap_rst_n,
    input wire s_axi_control_AWVALID,
    output wire s_axi_control_AWREADY,
    input wire [31:0] s_axi_control_AWADDR,
    input wire s_axi_control_WVALID,
    output wire s_axi_control_WREADY,
    input wire [31:0] s_axi_control_WDATA,
    input wire [3:0] s_axi_control_WSTRB,
    input wire s_axi_control_BREADY,
    output reg s_axi_control_BVALID,
    input wire s_axi_control_ARVALID,
    input wire [31:0] s_axi_control_ARADDR,
    input wire s_axi_control_RREADY,
    output reg interrupt,
    input wire [31:0] s_TDATA,
    input wire s_TVALID,
    output wire s_TREADY,
    input wire s_TLAST,
    output wire [31:0] result_TDATA,
    output wire result_TVALID,
    input wire result_TREADY,
    output wire result_TLAST
);
  reg started;
  reg [1:0] count;
  reg [31:0] sum;

  assign s_axi_control_AWREADY = 1'b1;
  assign s_axi_control_WREADY = 1'b1;

  // A receiver may wait for VALID before asserting READY. The output is
  // combinational, so the harness must settle both channels before sampling.
  assign s_TREADY = started && count < 2 && s_TVALID && result_TREADY;
  assign result_TVALID = started && count < 2 && s_TVALID;
  assign result_TDATA = sum + s_TDATA;
  assign result_TLAST = s_TLAST;

  always @(posedge ap_clk) begin
    if (!ap_rst_n) begin
      started <= 1'b0;
      count <= 2'd0;
      sum <= 32'd0;
      interrupt <= 1'b0;
      s_axi_control_BVALID <= 1'b0;
    end else begin
      if (s_axi_control_BVALID && s_axi_control_BREADY) begin
        s_axi_control_BVALID <= 1'b0;
      end
      if (s_axi_control_AWVALID && s_axi_control_WVALID && !s_axi_control_BVALID) begin
        s_axi_control_BVALID <= 1'b1;
        if (s_axi_control_AWADDR == 0 && s_axi_control_WDATA[0]) begin
          started <= 1'b1;
        end
      end
      if (s_TVALID && s_TREADY) begin
        sum <= sum + s_TDATA;
        count <= count + 1'b1;
        if (count == 1) begin
          interrupt <= 1'b1;
        end
      end
    end
  end
endmodule
