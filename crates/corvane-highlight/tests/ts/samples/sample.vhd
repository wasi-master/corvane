-- Debounced push-button counter driving four LEDs.
library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
entity button_counter is
  generic (CLK_HZ : positive := 50_000_000; DEBOUNCE_MS : natural := 10);
  port (
    clk, rst_n : in  std_logic;
    button     : in  std_logic;
    leds       : out std_logic_vector(3 downto 0)
  );
end entity button_counter;
architecture rtl of button_counter is
  constant LIMIT : natural := CLK_HZ / 1000 * DEBOUNCE_MS;
  type state_t is (IDLE, PRESSED, WAIT_RELEASE);
  signal state : state_t := IDLE;
  signal timer : natural range 0 to LIMIT := 0;
  signal count : unsigned(3 downto 0) := (others => '0');
  signal sync  : std_logic_vector(1 downto 0) := "00";
begin
  leds <= std_logic_vector(count) when state /= PRESSED else x"F";
  process (clk, rst_n)
  begin
    if rst_n = '0' then
      state <= IDLE;
      count <= (others => '0');
    elsif rising_edge(clk) then
      sync <= sync(0) & button;
      case state is
        when IDLE =>
          if sync(1) = '1' then state <= PRESSED; timer <= 0; end if;
        when PRESSED =>
          if timer = LIMIT then
            count <= count + 1;
            state <= WAIT_RELEASE;
          else
            timer <= timer + 1;
          end if;
        when WAIT_RELEASE =>
          if sync(1) = '0' then state <= IDLE; end if;
      end case;
    end if;
  end process;
  assert DEBOUNCE_MS < 1000 report "debounce too long" severity warning;
end architecture rtl;
