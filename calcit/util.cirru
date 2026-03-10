
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |util.core $ %{} :FileEntry
      :defs $ {}
        |inside-eval: $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro inside-eval: (& body)
              if
                = :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: eval") ~@body
                quasiquote $ do (println "|env: not eval. tests skipped")
          :examples $ []
          :schema $ :: :macro
            {} (:rest :dynamic)
              :args $ [] :dynamic
        |inside-js: $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro inside-js: (& body)
              if
                not= :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: js") ~@body
                quasiquote $ do (println "|env: not js. tests skipped")
          :examples $ []
          :schema $ :: :macro
            {} (:rest :dynamic)
              :args $ [] :dynamic
        |log-title $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ [] :dynamic
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () $ :: :unit
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
        |reload! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
          :schema $ :: :fn
            {} (:return :dynamic)
              :args $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns util.core $ :require
