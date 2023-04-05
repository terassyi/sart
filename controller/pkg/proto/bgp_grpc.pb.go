// Code generated by protoc-gen-go-grpc. DO NOT EDIT.
// versions:
// - protoc-gen-go-grpc v1.3.0
// - protoc             v3.6.1
// source: bgp.proto

package proto

import (
	context "context"
	empty "github.com/golang/protobuf/ptypes/empty"
	grpc "google.golang.org/grpc"
	codes "google.golang.org/grpc/codes"
	status "google.golang.org/grpc/status"
)

// This is a compile-time assertion to ensure that this generated file
// is compatible with the grpc package it is being compiled against.
// Requires gRPC-Go v1.32.0 or later.
const _ = grpc.SupportPackageIsVersion7

const (
	BgpApi_Health_FullMethodName      = "/sart.v1.BgpApi/Health"
	BgpApi_GetBgpInfo_FullMethodName  = "/sart.v1.BgpApi/GetBgpInfo"
	BgpApi_GetNeighbor_FullMethodName = "/sart.v1.BgpApi/GetNeighbor"
	BgpApi_GetPath_FullMethodName     = "/sart.v1.BgpApi/GetPath"
	BgpApi_SetAS_FullMethodName       = "/sart.v1.BgpApi/SetAS"
	BgpApi_SetRouterId_FullMethodName = "/sart.v1.BgpApi/SetRouterId"
	BgpApi_AddPeer_FullMethodName     = "/sart.v1.BgpApi/AddPeer"
	BgpApi_DeletePeer_FullMethodName  = "/sart.v1.BgpApi/DeletePeer"
	BgpApi_AddPath_FullMethodName     = "/sart.v1.BgpApi/AddPath"
	BgpApi_DeletePath_FullMethodName  = "/sart.v1.BgpApi/DeletePath"
)

// BgpApiClient is the client API for BgpApi service.
//
// For semantics around ctx use and closing/ending streaming RPCs, please refer to https://pkg.go.dev/google.golang.org/grpc/?tab=doc#ClientConn.NewStream.
type BgpApiClient interface {
	Health(ctx context.Context, in *HealthRequest, opts ...grpc.CallOption) (*empty.Empty, error)
	GetBgpInfo(ctx context.Context, in *GetBgpInfoRequest, opts ...grpc.CallOption) (*GetBgpInfoResponse, error)
	GetNeighbor(ctx context.Context, in *GetNeighborRequest, opts ...grpc.CallOption) (*GetNeighborResponse, error)
	// rpc ListNeighbor(ListNeighborRequest) returns (ListNeighborResponse);
	GetPath(ctx context.Context, in *GetPathRequest, opts ...grpc.CallOption) (*GetPathResponse, error)
	SetAS(ctx context.Context, in *SetASRequest, opts ...grpc.CallOption) (*empty.Empty, error)
	SetRouterId(ctx context.Context, in *SetRouterIdRequest, opts ...grpc.CallOption) (*empty.Empty, error)
	AddPeer(ctx context.Context, in *AddPeerRequest, opts ...grpc.CallOption) (*empty.Empty, error)
	DeletePeer(ctx context.Context, in *DeletePeerRequest, opts ...grpc.CallOption) (*empty.Empty, error)
	AddPath(ctx context.Context, in *AddPathRequest, opts ...grpc.CallOption) (*AddPathResponse, error)
	DeletePath(ctx context.Context, in *DeletePathRequest, opts ...grpc.CallOption) (*DeletePathResponse, error)
}

type bgpApiClient struct {
	cc grpc.ClientConnInterface
}

func NewBgpApiClient(cc grpc.ClientConnInterface) BgpApiClient {
	return &bgpApiClient{cc}
}

func (c *bgpApiClient) Health(ctx context.Context, in *HealthRequest, opts ...grpc.CallOption) (*empty.Empty, error) {
	out := new(empty.Empty)
	err := c.cc.Invoke(ctx, BgpApi_Health_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) GetBgpInfo(ctx context.Context, in *GetBgpInfoRequest, opts ...grpc.CallOption) (*GetBgpInfoResponse, error) {
	out := new(GetBgpInfoResponse)
	err := c.cc.Invoke(ctx, BgpApi_GetBgpInfo_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) GetNeighbor(ctx context.Context, in *GetNeighborRequest, opts ...grpc.CallOption) (*GetNeighborResponse, error) {
	out := new(GetNeighborResponse)
	err := c.cc.Invoke(ctx, BgpApi_GetNeighbor_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) GetPath(ctx context.Context, in *GetPathRequest, opts ...grpc.CallOption) (*GetPathResponse, error) {
	out := new(GetPathResponse)
	err := c.cc.Invoke(ctx, BgpApi_GetPath_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) SetAS(ctx context.Context, in *SetASRequest, opts ...grpc.CallOption) (*empty.Empty, error) {
	out := new(empty.Empty)
	err := c.cc.Invoke(ctx, BgpApi_SetAS_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) SetRouterId(ctx context.Context, in *SetRouterIdRequest, opts ...grpc.CallOption) (*empty.Empty, error) {
	out := new(empty.Empty)
	err := c.cc.Invoke(ctx, BgpApi_SetRouterId_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) AddPeer(ctx context.Context, in *AddPeerRequest, opts ...grpc.CallOption) (*empty.Empty, error) {
	out := new(empty.Empty)
	err := c.cc.Invoke(ctx, BgpApi_AddPeer_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) DeletePeer(ctx context.Context, in *DeletePeerRequest, opts ...grpc.CallOption) (*empty.Empty, error) {
	out := new(empty.Empty)
	err := c.cc.Invoke(ctx, BgpApi_DeletePeer_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) AddPath(ctx context.Context, in *AddPathRequest, opts ...grpc.CallOption) (*AddPathResponse, error) {
	out := new(AddPathResponse)
	err := c.cc.Invoke(ctx, BgpApi_AddPath_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

func (c *bgpApiClient) DeletePath(ctx context.Context, in *DeletePathRequest, opts ...grpc.CallOption) (*DeletePathResponse, error) {
	out := new(DeletePathResponse)
	err := c.cc.Invoke(ctx, BgpApi_DeletePath_FullMethodName, in, out, opts...)
	if err != nil {
		return nil, err
	}
	return out, nil
}

// BgpApiServer is the server API for BgpApi service.
// All implementations must embed UnimplementedBgpApiServer
// for forward compatibility
type BgpApiServer interface {
	Health(context.Context, *HealthRequest) (*empty.Empty, error)
	GetBgpInfo(context.Context, *GetBgpInfoRequest) (*GetBgpInfoResponse, error)
	GetNeighbor(context.Context, *GetNeighborRequest) (*GetNeighborResponse, error)
	// rpc ListNeighbor(ListNeighborRequest) returns (ListNeighborResponse);
	GetPath(context.Context, *GetPathRequest) (*GetPathResponse, error)
	SetAS(context.Context, *SetASRequest) (*empty.Empty, error)
	SetRouterId(context.Context, *SetRouterIdRequest) (*empty.Empty, error)
	AddPeer(context.Context, *AddPeerRequest) (*empty.Empty, error)
	DeletePeer(context.Context, *DeletePeerRequest) (*empty.Empty, error)
	AddPath(context.Context, *AddPathRequest) (*AddPathResponse, error)
	DeletePath(context.Context, *DeletePathRequest) (*DeletePathResponse, error)
	mustEmbedUnimplementedBgpApiServer()
}

// UnimplementedBgpApiServer must be embedded to have forward compatible implementations.
type UnimplementedBgpApiServer struct {
}

func (UnimplementedBgpApiServer) Health(context.Context, *HealthRequest) (*empty.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method Health not implemented")
}
func (UnimplementedBgpApiServer) GetBgpInfo(context.Context, *GetBgpInfoRequest) (*GetBgpInfoResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method GetBgpInfo not implemented")
}
func (UnimplementedBgpApiServer) GetNeighbor(context.Context, *GetNeighborRequest) (*GetNeighborResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method GetNeighbor not implemented")
}
func (UnimplementedBgpApiServer) GetPath(context.Context, *GetPathRequest) (*GetPathResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method GetPath not implemented")
}
func (UnimplementedBgpApiServer) SetAS(context.Context, *SetASRequest) (*empty.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method SetAS not implemented")
}
func (UnimplementedBgpApiServer) SetRouterId(context.Context, *SetRouterIdRequest) (*empty.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method SetRouterId not implemented")
}
func (UnimplementedBgpApiServer) AddPeer(context.Context, *AddPeerRequest) (*empty.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method AddPeer not implemented")
}
func (UnimplementedBgpApiServer) DeletePeer(context.Context, *DeletePeerRequest) (*empty.Empty, error) {
	return nil, status.Errorf(codes.Unimplemented, "method DeletePeer not implemented")
}
func (UnimplementedBgpApiServer) AddPath(context.Context, *AddPathRequest) (*AddPathResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method AddPath not implemented")
}
func (UnimplementedBgpApiServer) DeletePath(context.Context, *DeletePathRequest) (*DeletePathResponse, error) {
	return nil, status.Errorf(codes.Unimplemented, "method DeletePath not implemented")
}
func (UnimplementedBgpApiServer) mustEmbedUnimplementedBgpApiServer() {}

// UnsafeBgpApiServer may be embedded to opt out of forward compatibility for this service.
// Use of this interface is not recommended, as added methods to BgpApiServer will
// result in compilation errors.
type UnsafeBgpApiServer interface {
	mustEmbedUnimplementedBgpApiServer()
}

func RegisterBgpApiServer(s grpc.ServiceRegistrar, srv BgpApiServer) {
	s.RegisterService(&BgpApi_ServiceDesc, srv)
}

func _BgpApi_Health_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(HealthRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).Health(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_Health_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).Health(ctx, req.(*HealthRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_GetBgpInfo_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(GetBgpInfoRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).GetBgpInfo(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_GetBgpInfo_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).GetBgpInfo(ctx, req.(*GetBgpInfoRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_GetNeighbor_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(GetNeighborRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).GetNeighbor(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_GetNeighbor_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).GetNeighbor(ctx, req.(*GetNeighborRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_GetPath_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(GetPathRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).GetPath(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_GetPath_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).GetPath(ctx, req.(*GetPathRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_SetAS_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(SetASRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).SetAS(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_SetAS_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).SetAS(ctx, req.(*SetASRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_SetRouterId_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(SetRouterIdRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).SetRouterId(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_SetRouterId_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).SetRouterId(ctx, req.(*SetRouterIdRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_AddPeer_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AddPeerRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).AddPeer(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_AddPeer_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).AddPeer(ctx, req.(*AddPeerRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_DeletePeer_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(DeletePeerRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).DeletePeer(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_DeletePeer_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).DeletePeer(ctx, req.(*DeletePeerRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_AddPath_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(AddPathRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).AddPath(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_AddPath_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).AddPath(ctx, req.(*AddPathRequest))
	}
	return interceptor(ctx, in, info, handler)
}

func _BgpApi_DeletePath_Handler(srv interface{}, ctx context.Context, dec func(interface{}) error, interceptor grpc.UnaryServerInterceptor) (interface{}, error) {
	in := new(DeletePathRequest)
	if err := dec(in); err != nil {
		return nil, err
	}
	if interceptor == nil {
		return srv.(BgpApiServer).DeletePath(ctx, in)
	}
	info := &grpc.UnaryServerInfo{
		Server:     srv,
		FullMethod: BgpApi_DeletePath_FullMethodName,
	}
	handler := func(ctx context.Context, req interface{}) (interface{}, error) {
		return srv.(BgpApiServer).DeletePath(ctx, req.(*DeletePathRequest))
	}
	return interceptor(ctx, in, info, handler)
}

// BgpApi_ServiceDesc is the grpc.ServiceDesc for BgpApi service.
// It's only intended for direct use with grpc.RegisterService,
// and not to be introspected or modified (even as a copy)
var BgpApi_ServiceDesc = grpc.ServiceDesc{
	ServiceName: "sart.v1.BgpApi",
	HandlerType: (*BgpApiServer)(nil),
	Methods: []grpc.MethodDesc{
		{
			MethodName: "Health",
			Handler:    _BgpApi_Health_Handler,
		},
		{
			MethodName: "GetBgpInfo",
			Handler:    _BgpApi_GetBgpInfo_Handler,
		},
		{
			MethodName: "GetNeighbor",
			Handler:    _BgpApi_GetNeighbor_Handler,
		},
		{
			MethodName: "GetPath",
			Handler:    _BgpApi_GetPath_Handler,
		},
		{
			MethodName: "SetAS",
			Handler:    _BgpApi_SetAS_Handler,
		},
		{
			MethodName: "SetRouterId",
			Handler:    _BgpApi_SetRouterId_Handler,
		},
		{
			MethodName: "AddPeer",
			Handler:    _BgpApi_AddPeer_Handler,
		},
		{
			MethodName: "DeletePeer",
			Handler:    _BgpApi_DeletePeer_Handler,
		},
		{
			MethodName: "AddPath",
			Handler:    _BgpApi_AddPath_Handler,
		},
		{
			MethodName: "DeletePath",
			Handler:    _BgpApi_DeletePath_Handler,
		},
	},
	Streams:  []grpc.StreamDesc{},
	Metadata: "bgp.proto",
}