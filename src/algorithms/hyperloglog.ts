export class HyperLogLog {
	private registers: Uint8Array;
	private p: number;
	private m: number;
	private alphaMM: number;

	constructor(precision: number = 14) {
		// p in [4, 16] usually for HLL
		this.p = Math.max(4, Math.min(16, precision));
		this.m = 1 << this.p;
		this.registers = new Uint8Array(this.m);
		this.alphaMM = this.getAlpha() * this.m * this.m;
	}

	private getAlpha(): number {
		switch (this.p) {
			case 4:
				return 0.673;
			case 5:
				return 0.697;
			case 6:
				return 0.709;
			default:
				return 0.7213 / (1 + 1.079 / this.m);
		}
	}

	add(value: string): void {
		const hash = Bun.hash.xxHash64(value);
		// Use the first p bits for the index
		const registerIndex = Number(hash >> BigInt(64 - this.p));
		// Use the remaining 64-p bits for counting leading zeros
		// Mask the top p bits to 0, then count LZ
		// Or shift left by p to clear top bits, then count LZ of the result
		const remaining = hash << BigInt(this.p);
		
		// The original implementation +1 is correct because we want position of first 1.
		// If remaining is 0, it means all bits were 0 (very unlikely with 64-bit hash).
		const leadingZeros = this.countLeadingZeros(remaining) + 1;
		
		this.registers[registerIndex] = Math.max(
			this.registers[registerIndex],
			leadingZeros,
		);
	}

	private countLeadingZeros(value: bigint): number {
		if (value === 0n) return 64 - this.p; // Should be the remaining width
		let count = 0;
		let v = value;
		// Check top bit (bit 63)
		while ((v & 0x8000000000000000n) === 0n) {
			count++;
			v <<= 1n;
		}
		return count;
	}

	count(): number {
		const m = this.m;
		let rawSum = 0;
		let zeroRegisters = 0;

		for (let i = 0; i < m; i++) {
			rawSum += 1.0 / (1 << this.registers[i]);
			if (this.registers[i] === 0) zeroRegisters++;
		}

		const rawEstimate = this.alphaMM / rawSum;

		// Small range correction (Linear Counting)
		if (rawEstimate <= 2.5 * m) {
			if (zeroRegisters !== 0) {
				return m * Math.log(m / zeroRegisters);
			}
			return rawEstimate;
		}

		// Large range correction (for 32-bit hashes usually, but we have 64-bit)
		// For 64-bit hashes, the correction applies near 2^64, which is effectively unreachable.
		// However, the standard usually just returns rawEstimate if it's large but not "huge".
		// We can safely return rawEstimate for all practical log volume purposes.
		
		// If we were strictly 32-bit, the limit is 2^32/30.
		// Since we use 64-bit hash, we don't need the 32-bit collision correction.
		
		return rawEstimate;
	}

	merge(other: HyperLogLog): void {
		if (this.m !== other.m) {
			throw new Error("Cannot merge HyperLogLogs with different precision");
		}
		for (let i = 0; i < this.m; i++) {
			this.registers[i] = Math.max(this.registers[i], other.registers[i]);
		}
	}

	serialize(): Uint8Array {
		return new Uint8Array(this.registers);
	}

	static deserialize(data: Uint8Array, precision: number): HyperLogLog {
		const hll = new HyperLogLog(precision);
		hll.registers.set(data);
		return hll;
	}
}
