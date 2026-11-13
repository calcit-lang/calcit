
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |demo
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'demo.main/main!) (:mode :native) (:reload-fn 'demo.main/main!) (:target :native)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'demo.main
    %{} 'FileEntry
      :defs $ {}
        'Box $ %{} 'CodeEntry
          :doc "|Generic struct written with a `(:generics …)` clause."
          :code $ quote $ defstruct Box (:generics T) (:value T)
          :examples $ []
          :schema $ quote StructDef
        'Pair $ %{} 'CodeEntry
          :doc "|Generic enum written with a `(:generics …)` clause."
          :code $ quote $ defenum Pair (:generics A B) (:both A B) (:none)
          :examples $ []
          :schema $ quote EnumDef
        'Shown $ %{} 'CodeEntry
          :doc "|Bounded struct written with `(:generics …)` and `(:where …)`."
          :code $ quote $ defstruct Shown (:generics T)
            :where $ T Debug
            :value T
            :count Number 0
          :examples $ []
          :schema $ quote StructDef
        'box-value $ %{} 'CodeEntry (:doc "|Reads the payload of a `Box Number`.")
          :code $ quote $ defn box-value (b) (:value b)
          :examples $ []
          :schema $ quote $ Fn (Box Number) (-> Number)
        'first-of $ %{} 'CodeEntry (:doc "|Reads the first payload of a `Pair`.")
          :code $ quote $ defn first-of (p fallback)
            match p
              (:both a _b) a
              (:none) fallback
          :examples $ []
          :schema $ quote $ Fn (Pair Number String) Number (-> Number)
        'main! $ %{} 'CodeEntry (:doc |Entry.)
          :code $ quote $ defn main! ()
            println $ &+
              box-value $ Box :value 1
              first-of (Pair :both 2 |x) 0
            println $ :count $ Shown :value |a
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns demo.main
