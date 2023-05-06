package constants

import (
	"strings"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

const (
	MetaPrefix                 string = "sart.terassyi.net/"
	Namespace                  string = "kube-system"
	KubernetesServiceNameLabel string = "kubernetes.io/service-name"
)

const (
	LabelKeyAsn = MetaPrefix + "asn"
	Finalizer   = MetaPrefix + "finalizer"
)

const (
	AnnotationAddressPool       string = MetaPrefix + "address-pool"
	AnnotationAllocatedFromPool string = MetaPrefix + "allocated-from-pool"
)

const (
	ProtocolIpv4 string = "ipv4"
	ProtocolIpv6 string = "ipv6"
)

const (
	AddressPoolFinalizer = "addresspool" + "." + Finalizer
)

const (
	DefaultPeerStateWatchInterval uint64 = 5
)

func GetFinalizerName(obj metav1.TypeMeta) string {
	return strings.ToLower(obj.Kind) + "." + Finalizer
}
