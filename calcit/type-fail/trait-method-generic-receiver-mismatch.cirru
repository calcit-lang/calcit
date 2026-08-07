
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |type-fail-trait-method-generic-receiver) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn 'type-fail-trait-method-generic-receiver.main/main!) (:mode :native) (:reload-fn 'type-fail-trait-method-generic-receiver.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |type-fail-trait-method-generic-receiver.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc "|Entry for generic Option receiver method argument mismatch")
          :code $ quote
            defn main! (option) (option .unwrap-or 0)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ [] (:: 'Option 'String)
        |plus1 $ %{} :CodeEntry (:doc "|Schema expects :number, call-site passes :string")
          :code $ quote
            defn plus1 (x) (&+ x 1)
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Number)
              :args $ [] 'Number
        |reload! $ %{} :CodeEntry (:doc "|Reload handler")
          :code $ quote
            defn reload! () nil
          :examples $ []
          :schema $ :: 'Fn
            {} (:return 'Dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc "|Namespace for generic method receiver mismatch")
        :code $ quote (ns type-fail-trait-method-generic-receiver.main)
