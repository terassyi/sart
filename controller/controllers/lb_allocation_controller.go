package controllers

import (
	"context"
	"fmt"
	"net/netip"
	"reflect"
	"strings"

	"github.com/go-logr/logr"
	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/allocator"
	"github.com/terassyi/sart/controller/pkg/constants"
	v1 "k8s.io/api/core/v1"
	discoveryv1 "k8s.io/api/discovery/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

type LBAllocationReconciler struct {
	client.Client
	Allocators map[string]map[string]allocator.Allocator
}

// +kubebuilder:rbac:groups="",resources=services,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=services/status,verbs=get;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices,verbs=get;list;watch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools,verbs=get;list;watch;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/finalizers,verbs=update

func (r *LBAllocationReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	if _, err := r.reconcileAddressPool(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile AddressPool")
		return ctrl.Result{}, err
	}

	if _, err := r.reconcileService(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile Service")
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&v1.Service{}).
		Watches(&source.Kind{Type: &sartv1alpha1.AddressPool{}}, &handler.EnqueueRequestForObject{}).
		Watches(&source.Kind{Type: &discoveryv1.EndpointSlice{}}, handler.EnqueueRequestsFromMapFunc(func(o client.Object) []reconcile.Request {
			epSlice, ok := o.(*discoveryv1.EndpointSlice)
			if !ok {
				return []reconcile.Request{}
			}
			serviceName, err := serviceNameFromEndpointSlice(epSlice)
			if err != nil {
				return []reconcile.Request{}
			}
			return []reconcile.Request{{NamespacedName: serviceName}}
		})).
		Complete(r)
}

func (r *LBAllocationReconciler) reconcileAddressPool(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	addressPool := &sartv1alpha1.AddressPool{}
	if err := r.Get(ctx, req.NamespacedName, addressPool); err != nil {
		if apierrors.IsNotFound(err) {
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get addresspool", "name", addressPool.Name)
		return ctrl.Result{}, err
	}

	// add finalizer
	finalizer := constants.GetFinalizerName(addressPool.TypeMeta)
	if !controllerutil.ContainsFinalizer(addressPool, finalizer) {
		controllerutil.AddFinalizer(addressPool, finalizer)
		if err := r.Client.Update(ctx, addressPool); err != nil {
			return ctrl.Result{}, err
		}
	}
	// remove finalizer and delete addresspool
	if !addressPool.ObjectMeta.DeletionTimestamp.IsZero() {
		logger.Info("deletion timestamp is not zero. remove AddressPool", "Name", addressPool.Name, "DeletionTimestamp", addressPool.DeletionTimestamp)
		if controllerutil.ContainsFinalizer(addressPool, finalizer) {
			controllerutil.RemoveFinalizer(addressPool, finalizer)

			delete(r.Allocators, addressPool.Name)

			if err := r.Client.Update(ctx, addressPool); err != nil {
				logger.Error(err, "failed to remove finalizer")
				return ctrl.Result{}, err
			}
		}
		return ctrl.Result{}, nil
	}

	if addressPool.Spec.Type != "lb" {
		return ctrl.Result{}, nil
	}

	p, ok := r.Allocators[addressPool.Name]
	if !ok {
		// create AddressaddressPool
		protocolAllocator := make(map[string]allocator.Allocator)
		for _, cidr := range addressPool.Spec.Cidrs {
			prefix, err := netip.ParsePrefix(cidr.Prefix)
			if err != nil {
				logger.Error(err, "failed to parse CIDR", "AddressaddressPool.Spec.Cidr", cidr)
				return ctrl.Result{}, err
			}
			protocolAllocator[cidr.Protocol] = allocator.New(&prefix)
		}
		logger.Info("create an allocator", "CIDRs", addressPool.Spec.Cidrs, "Disabled", addressPool.Spec.Disable)
		r.Allocators[addressPool.Name] = protocolAllocator
		return ctrl.Result{}, nil
	}

	logger.Info("AddressaddressPool is already created", "AddressaddressPool", addressPool.Name, "allocator", p)
	for _, a := range p {
		if addressPool.Spec.Disable {
			a.Disable()
		} else {
			a.Enable()
		}
	}
	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) reconcileService(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	svc := &v1.Service{}
	if err := r.Client.Get(ctx, req.NamespacedName, svc); err != nil {
		if apierrors.IsNotFound(err) {
			// handle deletion
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get Service", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}

	// filter Load balancer type
	if svc.Spec.Type != v1.ServiceTypeLoadBalancer {
		return ctrl.Result{}, nil
	}
	// list endpoint slices
	epSliceList := discoveryv1.EndpointSliceList{}
	if err := r.Client.List(ctx, &epSliceList, client.MatchingLabels{constants.KubernetesServiceNameLabel: req.Name}); err != nil {
		logger.Error(err, "failed to list EndpointSlice by Service", "ServiceName", req.NamespacedName.String())
		return ctrl.Result{}, err
	}
	epNames := []string{}
	for _, eps := range epSliceList.Items {
		epNames = append(epNames, eps.Name)
	}
	logger.Info("endpoint slice list", "EndpointSlice", epNames)

	// allocation
	newSvc := svc.DeepCopy()

	poolName, allocatedAddrs, err := r.allocate(logger, newSvc, epSliceList)
	if err != nil {
		logger.Error(err, "failed to allocate a LB address", "Name", req.NamespacedName)
		// clear allocation information from svc
		if err := r.release(logger, newSvc); err != nil {
			logger.Error(err, "failed to release", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, err
	}
	if err := r.setAllocationInfo(newSvc, poolName, allocatedAddrs); err != nil {
		logger.Error(err, "failed to set allocation information", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}
	logger.Info("annoations", "Annations", newSvc.Annotations)

	if !reflect.DeepEqual(svc.Annotations, newSvc.Annotations) {
		// update all
		logger.Info("update service annotations and status")
		if err := r.Client.Update(ctx, newSvc); err != nil {
			logger.Error(err, "failed to update service", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, nil
	}
	if !reflect.DeepEqual(svc.Status, newSvc.Status) {
		// update status
		logger.Info("update service status")
		if err := r.Client.Status().Update(ctx, newSvc); err != nil {
			logger.Error(err, "failed to update service status", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, nil
	}

	// nothing to update
	logger.Info("nothing to update service", "Name", req.NamespacedName)
	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) allocate(logger logr.Logger, svc *v1.Service, eps discoveryv1.EndpointSliceList) (string, []netip.Addr, error) {

	// check protocol by clusterIP
	if len(svc.Spec.ClusterIP) == 0 && svc.Spec.ClusterIP == "" {
		return "", nil, fmt.Errorf("Service doesn't have ClusterIP")
	}

	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return "", nil, fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAddressPool)
	}

	lbIPs := []netip.Addr{}
	// check allocated LB IP
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return "", nil, err
		}
		lbIPs = append(lbIPs, addr)
	}

	desiredIPs := make(map[string]netip.Addr)
	if len(lbIPs) > 0 {
		logger.Info("LB address is already allocated", "Name", svc.Name, "Addresses", lbIPs)
		from, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
		if !ok {
			return "", nil, fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAllocatedFromPool)
		}
		if poolName != from {
			return "", nil, fmt.Errorf("required pool is %s, actual allocating pool is %s", poolName, from)
		}

		protocolAllocator, ok := r.Allocators[poolName]
		if !ok {
			return "", nil, fmt.Errorf("Allocator is not registered: %s", poolName)
		}
		for _, ip := range lbIPs {
			// specified lb address, but not allocated
			if ip.Is4() {
				if protocolAllocator[constants.ProtocolIpv4].IsAllocated(ip) {
					logger.Info("ipv4 address is allocated", "Pool", from, "Address", ip)
					break
				}
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				logger.Info("ipv6 address is allocated", "Pool", from, "Address", ip)
				if protocolAllocator[constants.ProtocolIpv6].IsAllocated(ip) {
					break
				}
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
		return poolName, lbIPs, nil
	}

	// get desired address from service spec
	// TODO: I'm not sure when we handle dual stack
	if svc.Spec.LoadBalancerIP != "" {
		logger.Info("Spec.LoadBalancer is specified", "LoadBalancerIP", svc.Spec.LoadBalancerIP)
		ipStrs := strings.Split(svc.Spec.LoadBalancerIP, ",")
		for _, is := range ipStrs {
			ip, err := netip.ParseAddr(is)
			if err != nil {
				return "", nil, err
			}
			if ip.Is4() {
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
	}

	// allocation
	logger.Info("allocate by each allocator", "Pool", poolName, "Address", desiredIPs)
	allocated := make(map[string]netip.Addr)
	for protocol, allocator := range r.Allocators[poolName] {
		desired, ok := desiredIPs[protocol]
		if ok {
			addr, err := allocator.Allocate(desired)
			if err != nil {
				return "", nil, err
			}
			logger.Info("Allocate from pool", "Pool", poolName, "Address", addr)
			// allocated = append(allocated, addr)
			allocated[protocol] = addr
		} else {
			addr, err := allocator.AllocateNext()
			if err != nil {
				return "", nil, err
			}
			logger.Info("Allocate next from pool", "Pool", poolName, "Address", addr)
			allocated[protocol] = addr
		}
	}

	logger.Info("allocate lb address", "Pool", poolName, "Addresses", allocated)
	a := make([]netip.Addr, 0, 2)
	for _, v := range allocated {
		a = append(a, v)
	}
	return poolName, a, nil
}

func (r *LBAllocationReconciler) release(logger logr.Logger, svc *v1.Service) error {
	delete(svc.Annotations, constants.AnnotationAllocatedFromPool)
	if svc.Status.LoadBalancer.Ingress == nil || len(svc.Status.LoadBalancer.Ingress) == 0 {
		return nil
	}
	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return fmt.Errorf("pool is not found")
	}
	allocator, ok := r.Allocators[poolName]
	if !ok {
		return fmt.Errorf("allocator is not registered")
	}

	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return err
		}
		if addr.Is4() {
			_, err := allocator[constants.ProtocolIpv4].Release(addr)
			if err != nil {
				return err
			}
		} else {
			_, err := allocator[constants.ProtocolIpv6].Release(addr)
			if err != nil {
				return err
			}
		}
	}
	svc.Status.LoadBalancer = v1.LoadBalancerStatus{}
	return nil
}

func (r *LBAllocationReconciler) setAllocationInfo(svc *v1.Service, poolName string, addrs []netip.Addr) error {
	p, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok || p != poolName {
		svc.Annotations[constants.AnnotationAllocatedFromPool] = poolName
	}
	addrMap := make(map[string]netip.Addr)
	for _, lbAddr := range svc.Status.LoadBalancer.Ingress {
		a, err := netip.ParseAddr(lbAddr.IP)
		if err != nil {
			return err
		}
		if a.Is4() {
			addrMap[constants.ProtocolIpv4] = a
		} else {
			addrMap[constants.ProtocolIpv6] = a
		}
	}
	for _, addr := range addrs {
		if addr.Is4() {
			addrMap[constants.ProtocolIpv4] = addr
		} else {
			addrMap[constants.ProtocolIpv6] = addr
		}
	}
	lbAddrs := []v1.LoadBalancerIngress{}
	for _, a := range addrMap {
		lbAddrs = append(lbAddrs, v1.LoadBalancerIngress{IP: a.String()})
	}
	svc.Status.LoadBalancer.Ingress = lbAddrs
	return nil
}

func (r *LBAllocationReconciler) clearAllocationInfo(svc *v1.Service) error {
	delete(svc.Annotations, constants.AnnotationAllocatedFromPool)
	svc.Status.LoadBalancer = v1.LoadBalancerStatus{}
	return nil
}

func serviceNameFromEndpointSlice(epSlice *discoveryv1.EndpointSlice) (types.NamespacedName, error) {
	if epSlice == nil {
		return types.NamespacedName{}, fmt.Errorf("EndpointSlice is nil")
	}
	svcName, ok := epSlice.Labels[discoveryv1.LabelServiceName]
	if !ok || svcName == "" {
		return types.NamespacedName{}, fmt.Errorf("ServiceName is not found in labels")
	}
	return types.NamespacedName{Namespace: epSlice.Namespace, Name: svcName}, nil
}
