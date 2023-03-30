package allocator

type Allocator interface {
}

func New() Allocator {
	return nil
}

type allocator struct {
}
