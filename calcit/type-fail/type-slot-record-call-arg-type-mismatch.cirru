
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |type-fail-type-slot-record-call-arg)
  :configs $ {} (:init-fn |type-fail-type-slot-record-call-arg.main/main!) (:reload-fn |type-fail-type-slot-record-call-arg.main/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |type-fail-type-slot-record-call-arg.main $ %{} :FileEntry
      :defs $ {}
        |User $ %{} :CodeEntry (:doc "|Record type used for bind-type fixture")
          :code $ quote
            defstruct User (:name :string)
          :examples $ []
          :schema :dynamic
        |main! $ %{} :CodeEntry (:doc "|Entry for type-slot record bind call-site arg type mismatch")
          :code $ quote
            defn main! () $ with-type-slot (:payload $ %{} User (:name |demo))
              takes-user 1
              , nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |takes-user $ %{} :CodeEntry (:doc "|Schema expects a value matching the bound type slot")
          :code $ quote
            defn takes-user (x) x
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] '*payload
      :ns $ %{} :NsEntry (:doc "|Namespace for type-slot record call-site arg mismatch")
        :code $ quote (ns type-fail-type-slot-record-call-arg.main)