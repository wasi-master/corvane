// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {IERC20} from "./interfaces/IERC20.sol";

/// @title A simple time-locked vault
/// @notice Deposits unlock after a fixed delay.
contract Vault {
    error TooEarly(uint256 unlockAt);
    event Deposited(address indexed owner, uint256 amount, uint64 unlockAt);
    struct Lock { uint256 amount; uint64 unlockAt; }
    enum Status { Open, Paused }

    uint64 public constant DELAY = 7 days;
    IERC20 public immutable token;
    address private owner;
    Status public status = Status.Open;
    mapping(address => Lock) public locks;

    modifier onlyOwner() { require(msg.sender == owner, "Vault: not owner"); _; }

    constructor(IERC20 _token) { token = _token; owner = msg.sender; }

    /* The unlock time is reset on every deposit. */
    function deposit(uint256 amount) external {
        require(status == Status.Open && amount > 0, "Vault: closed or zero");
        Lock storage l = locks[msg.sender];
        l.amount += amount;
        l.unlockAt = uint64(block.timestamp) + DELAY;
        require(token.transferFrom(msg.sender, address(this), amount), "Vault: transfer failed");
        emit Deposited(msg.sender, amount, l.unlockAt);
    }

    function withdraw() external returns (uint256 amount) {
        Lock memory l = locks[msg.sender];
        if (block.timestamp < l.unlockAt) revert TooEarly(l.unlockAt);
        delete locks[msg.sender];
        amount = l.amount;
        require(token.transfer(msg.sender, amount), "Vault: transfer failed");
    }

    function setPaused(bool paused) external onlyOwner {
        status = paused ? Status.Paused : Status.Open;
    }
}
