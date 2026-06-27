{{- define "vantage.name" -}}{{ .Release.Name }}{{- end -}}
{{- define "vantage.dbConfig" -}}{{ .Release.Name }}-db-config{{- end -}}
{{- define "vantage.dbData" -}}{{ .Release.Name }}-db-data{{- end -}}
{{- define "vantage.hub" -}}{{ .Release.Name }}-hub{{- end -}}

{{- /*
  In-cluster DB password: use the value if set, else reuse the one already stored
  in the Secret (so `helm upgrade` keeps it), else generate a random one. Memoized
  into .Values so every call in this render returns the same value.
*/ -}}
{{- define "vantage.dbPassword" -}}
{{- if not .Values.timescaledb._pw -}}
  {{- $pw := .Values.timescaledb.password -}}
  {{- if not $pw -}}
    {{- $existing := lookup "v1" "Secret" .Release.Namespace (include "vantage.name" .) -}}
    {{- if and $existing $existing.data (hasKey $existing.data "db-password") -}}
      {{- $pw = index $existing.data "db-password" | b64dec -}}
    {{- else -}}
      {{- $pw = randAlphaNum 24 -}}
    {{- end -}}
  {{- end -}}
  {{- $_ := set .Values.timescaledb "_pw" $pw -}}
{{- end -}}
{{- .Values.timescaledb._pw -}}
{{- end -}}

{{- define "vantage.configUrl" -}}
{{- if .Values.timescaledb.enabled -}}
postgres://vantage:{{ include "vantage.dbPassword" . }}@{{ include "vantage.dbConfig" . }}:5432/vantage_config
{{- else -}}{{ .Values.hub.configDatabaseUrl }}{{- end -}}
{{- end -}}

{{- define "vantage.dataUrl" -}}
{{- if .Values.timescaledb.enabled -}}
postgres://vantage:{{ include "vantage.dbPassword" . }}@{{ include "vantage.dbData" . }}:5432/vantage_data
{{- else -}}{{ .Values.hub.dataDatabaseUrl }}{{- end -}}
{{- end -}}

{{- define "vantage.pullSecrets" -}}
{{- with .Values.image.pullSecrets }}
imagePullSecrets:
{{- range . }}
  - name: {{ . }}
{{- end }}
{{- end }}
{{- end -}}
