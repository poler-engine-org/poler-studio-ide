// ============================================================================
// POLER-FPGA CORE: Multiplier-Less Shift-and-Add & Polar Inversion
// Target Hardware: Microchip PolarFire SoC / Lattice iCE40
// ============================================================================

`timescale 1ns / 1ps

module poler_fpga_core (
    input  wire        clk,
    input  wire        rst_n,
    input  wire [127:0] state_p_in,
    input  wire [15:0]  epsilon_density,
    output reg  [127:0] state_p_out,
    output reg         stationarity_flag
);

    reg [127:0] resonance_accum;
    reg [7:0]   cycle_count;

    // Shift-and-add constant multiplier for decay rho = 0.9 (approx 230/256)
    wire [127:0] rho_decay_val = (resonance_accum >> 1) + (resonance_accum >> 2) + (resonance_accum >> 4);

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            state_p_out       <= 128'd0;
            resonance_accum   <= 128'd0;
            stationarity_flag <= 1'b0;
            cycle_count       <= 8'd0;
        end else begin
            // 1. IIR Resonance Accumulator R_t = eps_t + rho * R_{t-1}
            resonance_accum <= {112'd0, epsilon_density} + rho_decay_val;
            
            // 2. State Evolution
            state_p_out <= state_p_in ^ resonance_accum;
            
            // 3. Stationarity check
            if (epsilon_density < 16'd10) begin
                stationarity_flag <= 1'b1;
            end else begin
                stationarity_flag <= 1'b0;
            end
            
            cycle_count <= cycle_count + 1'b1;
        end
    end

endmodule
