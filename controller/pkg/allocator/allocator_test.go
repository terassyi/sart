package allocator

import (
	"net/netip"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestAllocator_allocate_release(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name string
		cidr netip.Prefix
		addr netip.Addr
	}{
		{
			name: "10.0.0.0/24_10.0.0.1",
			cidr: parseCIDRWithoutErr("10.0.0.0/24"),
			addr: parseAddrWithoutErr("10.0.0.1"),
		},
		{
			name: "10.0.0.0/32_10.0.0.0",
			cidr: parseCIDRWithoutErr("10.0.0.0/32"),
			addr: parseAddrWithoutErr("10.0.0.0"),
		},
		{
			name: "58.3.206.0/25_58.3.206.84",
			cidr: parseCIDRWithoutErr("58.3.206.0/25"),
			addr: parseAddrWithoutErr("58.3.206.84"),
		},
		{
			name: "58.3.206.80/29_58.3.206.84",
			cidr: parseCIDRWithoutErr("58.3.206.80/29"),
			addr: parseAddrWithoutErr("58.3.206.84"),
		},
		{
			name: "58.3.206.84/31_58.3.206.84",
			cidr: parseCIDRWithoutErr("58.3.206.84/31"),
			addr: parseAddrWithoutErr("58.3.206.84"),
		},
		{
			name: "58.3.206.85/31_58.3.206.84",
			cidr: parseCIDRWithoutErr("58.3.206.84/31"),
			addr: parseAddrWithoutErr("58.3.206.85"),
		},
		{
			name: "2001:db8::/32_2001:db8::9d",
			cidr: parseCIDRWithoutErr("2001:db8::/32"),
			addr: parseAddrWithoutErr("2001:db8::9d"),
		},
		{
			name: "2001:0db8:0000:0000:0000:0000:0000:0080/121_2001:db8::9d",
			cidr: parseCIDRWithoutErr("2001:0db8:0000:0000:0000:0000:0000:0080/121"),
			addr: parseAddrWithoutErr("2001:db8::9d"),
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			a := newAllocator(&tt.cidr)
			r, err := a.allocate(tt.addr)
			require.NoError(t, err)
			assert.True(t, r)
			assert.True(t, a.isAllocated(tt.addr))

			rr, err := a.release(tt.addr)
			require.NoError(t, err)
			assert.True(t, rr)
			assert.False(t, a.isAllocated(tt.addr))
		})
	}
}

func TestAllocator_AllocateNext(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name    string
		cidr    netip.Prefix
		addrs   []netip.Addr
		next    netip.Addr
		wantErr error
	}{
		{
			name: "10.69.0.0/24",
			cidr: parseCIDRWithoutErr("10.69.0.0/24"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("10.69.0.0"),
				parseAddrWithoutErr("10.69.0.10"),
				parseAddrWithoutErr("10.69.0.11"),
			},
			next: parseAddrWithoutErr("10.69.0.1"),
		},
		{
			name: "10.69.0.4/31",
			cidr: parseCIDRWithoutErr("10.69.0.4/31"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("10.69.0.4"),
			},
			next: parseAddrWithoutErr("10.69.0.5"),
		},
		{
			name: "10.69.0.4/31",
			cidr: parseCIDRWithoutErr("10.69.0.4/31"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("10.69.0.5"),
			},
			next: parseAddrWithoutErr("10.69.0.4"),
		},
		{
			name: "10.69.0.4/31_full",
			cidr: parseCIDRWithoutErr("10.69.0.4/31"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("10.69.0.5"),
				parseAddrWithoutErr("10.69.0.4"),
			},
			next:    parseAddrWithoutErr("10.69.0.4"),
			wantErr: ErrRangeFull,
		},
		{
			name: "2001:db8:abcd:1234::/64",
			cidr: parseCIDRWithoutErr("2001:db8:abcd:1234::/64"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("2001:db8:abcd:1234::1"),
				parseAddrWithoutErr("2001:db8:abcd:1234::100"),
			},
			next: parseAddrWithoutErr("2001:db8:abcd:1234::"),
		},
		{
			name: "2001:db8::/127",
			cidr: parseCIDRWithoutErr("2001:db8::/127"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("2001:db8::"),
			},
			next: parseAddrWithoutErr("2001:db8::1"),
		},
		{
			name: "2001:db8::/127 full",
			cidr: parseCIDRWithoutErr("2001:db8::/127"),
			addrs: []netip.Addr{
				parseAddrWithoutErr("2001:db8::"),
				parseAddrWithoutErr("2001:db8::1"),
			},
			next:    parseAddrWithoutErr("2001:db8::1"),
			wantErr: ErrRangeFull,
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			a := newAllocator(&tt.cidr)
			for _, addr := range tt.addrs {
				r, err := a.allocate(addr)
				require.NoError(t, err)
				assert.True(t, r)
			}

			r, err := a.AllocateNext()
			if tt.wantErr == nil {
				require.NoError(t, err)
				assert.True(t, r)
				assert.True(t, a.isAllocated(tt.next))
			} else {
				require.Error(t, err, ErrRangeFull)
			}
		})
	}
}

func TestAllocator_isAllocated(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name      string
		cidr      netip.Prefix
		allocated bool
		addr      netip.Addr
	}{
		{
			name:      "allocated",
			cidr:      parseCIDRWithoutErr("10.69.0.1/32"),
			allocated: true,
			addr:      parseAddrWithoutErr("10.69.0.1"),
		},
		{
			name:      "not allocated",
			cidr:      parseCIDRWithoutErr("10.69.0.1/32"),
			allocated: false,
			addr:      parseAddrWithoutErr("10.69.0.1"),
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			allocator := newAllocator(&tt.cidr)
			if tt.allocated {
				_, err := allocator.allocate(tt.addr)
				require.NoError(t, err)
			}
			actual := allocator.isAllocated(tt.addr)
			assert.Equal(t, tt.allocated, actual)
		})
	}
}

func TestDiffAddr(t *testing.T) {
	t.Parallel()
	for _, tt := range []struct {
		name string
		a    netip.Addr
		b    netip.Addr
		want int64
	}{
		{
			name: "10.0.0.2 - 10.0.0.0",
			a:    netip.AddrFrom4([4]byte{10, 0, 0, 2}),
			b:    netip.AddrFrom4([4]byte{10, 0, 0, 0}),
			want: 2,
		},
		{
			name: "10.0.0.100 - 10.0.0.0",
			a:    netip.AddrFrom4([4]byte{10, 0, 0, 100}),
			b:    netip.AddrFrom4([4]byte{10, 0, 0, 0}),
			want: 100,
		},
		{
			name: "192.168.1.10 - 192.168.0.0",
			a:    netip.AddrFrom4([4]byte{192, 168, 1, 10}),
			b:    netip.AddrFrom4([4]byte{192, 168, 0, 0}),
			want: 256 + 10,
		},
		{
			name: "fe80::1 - fe80::",
			a:    parseAddrWithoutErr("fe80::1"),
			b:    parseAddrWithoutErr("fe80::"),
			want: 1,
		},
	} {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			assert.Equal(t, tt.want, diffAddr(tt.a, tt.b))
		})
	}
}

func parseAddrWithoutErr(addr string) netip.Addr {
	a, _ := netip.ParseAddr(addr)
	return a
}

func parseCIDRWithoutErr(cidr string) netip.Prefix {
	c, _ := netip.ParsePrefix(cidr)
	return c
}
