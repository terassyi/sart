package allocator

import (
	"errors"
	"fmt"
	"math/big"
	"net/netip"

	"github.com/bits-and-blooms/bitset"
)

var (
	ErrNotCIDRRange     error = errors.New("Address is contained in CIDR")
	ErrRangeFull        error = errors.New("Address range is full")
	ErrAlreadyAllocated error = errors.New("Already allocated")
	ErrNotAllocated     error = errors.New("Not allocated")
)

type Allocator interface {
	Allocate(netip.Addr) (netip.Addr, error)
	AllocateNext() (netip.Addr, error)
	Release(netip.Addr) (bool, error)
	IsAllocated(netip.Addr) bool
	IsEnabled() bool
	Enable()
	Disable()
	Allocated() string
}

func New(cidr *netip.Prefix) Allocator {
	return newAllocator(cidr)
}

type allocator struct {
	cidr    *netip.Prefix
	bits    *bitset.BitSet
	enabled bool
}

func newAllocator(cidr *netip.Prefix) *allocator {
	b := func() uint {
		if cidr.Addr().Is4() {
			return 32
		} else {
			return 128
		}
	}()
	return &allocator{
		cidr:    cidr,
		bits:    bitset.New(uint(1) << (b - uint(cidr.Bits()))),
		enabled: true,
	}
}

func (a *allocator) IsEnabled() bool {
	return a.enabled
}

func (a *allocator) Enable() {
	a.enabled = true
}

func (a *allocator) Disable() {
	a.enabled = false
}

func (a *allocator) Allocate(addr netip.Addr) (netip.Addr, error) {
	if !a.cidr.Contains(addr) {
		return netip.Addr{}, ErrNotCIDRRange
	}
	if a.isFull() {
		return netip.Addr{}, ErrRangeFull
	}
	return a.allocate(addr)
}

func (a *allocator) AllocateNext() (netip.Addr, error) {
	if a.isFull() {
		return netip.Addr{}, ErrRangeFull
	}
	offset, ok := a.bits.NextClear(0)
	if !ok {
		return netip.Addr{}, fmt.Errorf("failed to allocate")
	}
	a.bits.Set(offset)
	return fromOffset(*a.cidr, offset)
}

func (a *allocator) Release(addr netip.Addr) (bool, error) {
	if !a.cidr.Contains(addr) {
		return false, ErrNotCIDRRange
	}
	if a.isEmpty() {
		return false, ErrNotAllocated
	}
	return a.release(addr)
}

func (a *allocator) IsAllocated(addr netip.Addr) bool {
	return a.isAllocated(addr)
}

func (a *allocator) Allocated() string {
	return a.bits.DumpAsBits()
}

func (a *allocator) allocate(addr netip.Addr) (netip.Addr, error) {
	if a.isAllocated(addr) {
		return netip.Addr{}, ErrAlreadyAllocated
	}
	offset := diffAddr(addr, a.cidr.Addr())
	a.bits.Set(uint(offset))
	return addr, nil
}

func (a *allocator) release(addr netip.Addr) (bool, error) {
	if !a.isAllocated(addr) {
		return false, ErrNotAllocated
	}
	offset := diffAddr(addr, a.cidr.Addr())
	a.bits.Clear(uint(offset))
	return true, nil
}

func (a *allocator) isFull() bool {
	return a.bits.All()
}

func (a *allocator) isEmpty() bool {
	return a.bits.None()
}

func (a *allocator) isAllocated(addr netip.Addr) bool {
	if !a.cidr.Contains(addr) {
		return false
	}
	offset := diffAddr(addr, a.cidr.Addr())
	return a.bits.Test(uint(offset))
}

func diffAddr(a, b netip.Addr) int64 {
	aa := new(big.Int)
	bb := new(big.Int)
	aa.SetBytes(a.AsSlice())
	bb.SetBytes(b.AsSlice())
	return aa.Sub(aa, bb).Int64()
}

func fromOffset(cidr netip.Prefix, offset uint) (netip.Addr, error) {
	base := cidr.Addr()
	b := new(big.Int)
	b.SetBytes(base.AsSlice())
	bb := b.Add(b, big.NewInt(int64(offset))).Bytes()
	addr, ok := netip.AddrFromSlice(bb)
	if !ok {
		return netip.Addr{}, fmt.Errorf("invalid offset to CIDR: %s", cidr.String())
	}
	return addr, nil
}
