
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |defprobe
  :entries $ {} $ :default
    {} (:description |probe) (:init-fn 'defprobe.main/main!) (:mode :native) (:reload-fn 'defprobe.main/reload!)
      :feature-policy $ {} $ :js-ffi :error
      :modules $ []
      :type-slots $ {}
  :files $ {}
    'defprobe.macros $ %{} 'FileEntry
      :defs $ {}
        'DomElement $ %{} 'CodeEntry (:doc |)
          :code $ quote $ deftrait DomElement
            .select! $ quote $ Fn defprobe.macros/DomElement (-> Unit)
          :examples $ []
          :ffi $ {} (:backend :js) (:kind :external-object)
            :names $ {} $ :select! |select
          :schema $ quote Trait
        'Effect $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defstruct Effect (:name 'Tag) (:coord 'List) (:args 'List) (:method 'Fn)
          :examples $ []
          :schema $ quote StructDef
        'defeffect $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro defeffect (effect-name args params & body)
            let
                args-var $ gensym |args
                params-var $ gensym |params
              quasiquote $ defn ~effect-name ~args $ %{} defprobe.macros/Effect
                :name $ turn-tag effect-name
                :coord $ []
                :args $ [] ~@args
                :method $ fn (~args-var ~params-var)
                  let[] ~args ~args-var $ let[] ~params ~params-var $ ~
                    quasiquote $ do ~@body
          :examples $ []
          :schema $ quote $ Macro SyntaxSymbol SyntaxList SyntaxList & Syntax
            -> $ Definition Fn
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns defprobe.macros
    'defprobe.main $ %{} 'FileEntry
      :defs $ {}
        'effect-focus $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defprobe.macros/defeffect effect-focus (pattern) (action el at-place?)
            when (= action :mount)
              .select! $ unsafe-coerce el 'defprobe.macros/DomElement
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> defprobe.macros/Effect) (:features :js-ffi)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! ()
            hint-fn $ Fn $ -> Unit
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! ()
            hint-fn $ Fn $ -> Unit
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns defprobe.main
          :require $ defprobe.macros :refer $ defeffect
