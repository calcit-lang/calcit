
{} (:about "|file is generated - never edit directly; learn cr edit/tree workflows before changing") (:package |util)
  :configs $ {} (:init-fn |util.core/main!) (:reload-fn |util.core/reload!) (:version |0.0.0)
    :modules $ []
  :entries $ {}
  :files $ {}
    |util.core $ %{} :FileEntry
      :defs $ {}
        |inside-eval: $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defmacro inside-eval: (& body)
              if
                = :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: eval") ~@body
                quasiquote $ do (println "|env: not eval. tests skipped")
          :examples $ []
        |inside-js: $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defmacro inside-js: (& body)
              if
                not= :eval $ &get-calcit-running-mode
                quasiquote $ do (println "|env: js") ~@body
                quasiquote $ do (println "|env: not js. tests skipped")
          :examples $ []
        |log-title $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn log-title (title) (println) (println title) (println)
          :examples $ []
        |main! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn main! () $ :: :unit
          :examples $ []
        |reload! $ %{} :CodeEntry (:doc |) (:schema nil)
          :code $ quote
            defn reload! () $ :: :unit
          :examples $ []
      :ns $ %{} :CodeEntry (:doc |) (:schema nil)
        :code $ quote
          ns util.core $ :require
        :examples $ []
