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

func GetFinalizerName(obj metav1.TypeMeta) string {
	return strings.ToLower(obj.Kind) + "." + Finalizer
}
