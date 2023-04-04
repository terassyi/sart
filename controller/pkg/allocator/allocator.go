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
	Allocate(netip.Addr) (bool, error)
	AllocateNext() (bool, error)
	Release(netip.Addr) (bool, error)
	IsAllocated(netip.Addr) bool
	IsEnabled() bool
	Enable()
	Disable()
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

func (a *allocator) Allocate(addr netip.Addr) (bool, error) {
	if !a.cidr.Contains(addr) {
		return false, ErrNotCIDRRange
	}
	if a.isFull() {
		return false, ErrRangeFull
	}
	return a.allocate(addr)
}

func (a *allocator) AllocateNext() (bool, error) {
	if a.isFull() {
		return false, ErrRangeFull
	}
	offset, ok := a.bits.NextClear(0)
	if !ok {
		return false, fmt.Errorf("failed to allocate")
	}
	a.bits.Set(offset)
	return true, nil
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

func (a *allocator) allocate(addr netip.Addr) (bool, error) {
	if a.isAllocated(addr) {
		return false, ErrAlreadyAllocated
	}
	offset := diffAddr(addr, a.cidr.Addr())
	a.bits.Set(uint(offset))
	return true, nil
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
